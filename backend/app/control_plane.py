"""Buddy control plane — one message entry, one Cloud AI turn, deterministic handlers."""

from __future__ import annotations

import logging
import uuid
from datetime import datetime, timezone
from typing import Any, Protocol

from .ai.groq_provider import GroqError, GroqProvider
from .ai.prompt import SYSTEM_PROMPT, build_user_payload
from .buddy_turn import parse_buddy_turn
from .calendar import CalendarService
from .config import Settings, load_settings
from .db import get_connection, init_db
from .goals import GoalStore, progress_summary
from .migrations import run_migrations
from .planning import propose_for_goal
from .schemas import (
    BuddyTurn,
    ChatResponse,
    Goal,
    GoalUpdate,
    RequestedAction,
    SessionOut,
    Spark,
    TodayResponse,
)
from .sparks import SparkService

log = logging.getLogger("buddy.control_plane")


class AIClient(Protocol):
    def complete_json(self, system: str, user: str, *, allow_retry: bool = True) -> dict: ...

    def close(self) -> None: ...


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class ControlPlane:
    def __init__(
        self,
        *,
        db_path=None,
        settings: Settings | None = None,
        ai: AIClient | None = None,
    ):
        self.settings = settings or load_settings()
        path = db_path or self.settings.db_path
        self.conn = get_connection(path)
        init_db(self.conn)
        run_migrations(self.conn)
        self.goals = GoalStore(self.conn)
        self.calendar = CalendarService(self.conn)
        self.sparks = SparkService(self.conn)
        self._ai = ai
        self._ai_owned = ai is None
        self._call_count = 0  # per-process counter for tests

    def close(self) -> None:
        if self._ai_owned and self._ai is not None:
            self._ai.close()
        self.conn.close()

    @property
    def ai_available(self) -> bool:
        return bool(self.settings.groq_configured or (self._ai is not None and not self._ai_owned))

    def _provider(self) -> AIClient:
        if self._ai is None:
            if not self.settings.groq_configured:
                raise GroqError("config", "Cloud AI is not configured")
            self._ai = GroqProvider(self.settings)
        return self._ai

    # --- conversations / messages ---

    def _ensure_conversation(self, conversation_id: str | None) -> str:
        if conversation_id:
            row = self.conn.execute(
                "SELECT id FROM conversations WHERE id = ?", (conversation_id,)
            ).fetchone()
            if row:
                return conversation_id
        cid = _new_id()
        now = _now()
        self.conn.execute(
            "INSERT INTO conversations (id, created_at, updated_at) VALUES (?, ?, ?)",
            (cid, now, now),
        )
        self.conn.commit()
        return cid

    def _add_message(self, conversation_id: str, role: str, content: str) -> None:
        self.conn.execute(
            """
            INSERT INTO messages (id, conversation_id, role, content, created_at)
            VALUES (?, ?, ?, ?, ?)
            """,
            (_new_id(), conversation_id, role, content, _now()),
        )
        self.conn.execute(
            "UPDATE conversations SET updated_at=? WHERE id=?",
            (_now(), conversation_id),
        )
        self.conn.commit()

    def list_messages(self, conversation_id: str) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            """
            SELECT id, conversation_id, role, content, created_at
            FROM messages WHERE conversation_id=? ORDER BY created_at ASC
            """,
            (conversation_id,),
        ).fetchall()
        return [dict(r) for r in rows]

    def get_goal(self, goal_id: str) -> Goal | None:
        return self.goals.get(goal_id)

    # --- explicit UI actions (no AI) ---

    def decide_proposal(self, batch_id: str, decision: str) -> dict[str, Any]:
        if decision == "approve":
            booked = self.calendar.approve_batch(batch_id)
            return {"booked": booked}
        if decision == "reject":
            n = self.calendar.reject_batch(batch_id)
            return {"rejected": n}
        return {"ok": False, "detail": "Use chat to describe an adjustment"}

    def mark_outcome(self, session_id: str, outcome: str, notes: str | None = None):
        return self.calendar.mark_outcome(session_id, outcome, notes)

    # --- chat entry ---

    def handle_message(self, message: str, conversation_id: str | None = None) -> ChatResponse:
        text = message.strip()
        cid = self._ensure_conversation(conversation_id)
        self._add_message(cid, "user", text)

        # Deterministic short-circuit for explicit approve/reject words when a batch is open.
        goal = self.goals.active_for_conversation(cid)
        if goal:
            batch_id = self.calendar.open_proposal_batch(goal.id)
            lower = text.lower().strip()
            if batch_id and lower in {"approve", "approved", "yes approve", "looks good"}:
                booked = self.calendar.approve_batch(batch_id)
                reply = (
                    f"Booked {len(booked)} session(s) once each and linked them to {goal.title}. "
                    "Tell me when one is completed or missed and I'll record it."
                )
                self._add_message(cid, "assistant", reply)
                return ChatResponse(
                    conversation_id=cid,
                    reply=reply,
                    goal=self.goals.get(goal.id),
                    booked_sessions=booked,
                    ai_available=True,
                )
            if batch_id and lower in {"reject", "reject those", "no thanks", "reject that"}:
                n = self.calendar.reject_batch(batch_id)
                reply = f"Rejected {n} proposed session(s). Say what to change and I'll propose again."
                self._add_message(cid, "assistant", reply)
                return ChatResponse(conversation_id=cid, reply=reply, goal=goal, ai_available=True)

        if not self.settings.groq_configured and self._ai is None:
            reply = (
                "Today, Calendar and Sparks still work offline. "
                "Conversational understanding needs Cloud AI — add GROQ_API_KEY "
                "(and enable Zero Data Retention in the Groq console) to continue planning in Chat."
            )
            self._add_message(cid, "assistant", reply)
            return ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                ai_available=False,
            )

        try:
            turn = self._run_ai_turn(cid, text, goal)
        except GroqError as exc:
            reply = (
                "I couldn't reach Cloud AI just now, so I didn't change your plan. "
                f"({exc.category.replace('_', ' ')}) "
                "Today, Calendar and Sparks remain available."
            )
            self._add_message(cid, "assistant", reply)
            return ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                ai_available=False,
            )

        return self._apply_turn(cid, turn, goal)

    def _run_ai_turn(self, cid: str, text: str, goal: Goal | None) -> BuddyTurn:
        recent = [
            {"role": m["role"], "content": m["content"]}
            for m in self.list_messages(cid)[-12:]
        ]
        batch_id = self.calendar.open_proposal_batch(goal.id) if goal else None
        sparks = [
            {"id": s.id, "content": s.content}
            for s in self.sparks.list_open()[:8]
        ]
        payload = build_user_payload(
            message=text,
            active_goal=goal.model_dump() if goal else None,
            recent_messages=recent,
            open_proposal_batch_id=batch_id,
            open_sparks=sparks,
        )
        self._call_count += 1
        raw = self._provider().complete_json(SYSTEM_PROMPT, payload, allow_retry=True)
        try:
            return parse_buddy_turn(raw)
        except Exception:
            # One repair retry for malformed structured output (counts as the allowed retry path).
            self._call_count += 1
            repair = (
                payload
                + "\n\nPrevious output was invalid. Return ONLY valid BuddyTurn JSON using exact intent enums."
            )
            raw2 = self._provider().complete_json(SYSTEM_PROMPT, repair, allow_retry=False)
            try:
                return parse_buddy_turn(raw2)
            except Exception as exc:  # noqa: BLE001
                raise GroqError("malformed", "Cloud AI returned an invalid BuddyTurn") from exc

    def _apply_turn(self, cid: str, turn: BuddyTurn, goal: Goal | None) -> ChatResponse:
        proposed: list[SessionOut] = []
        booked: list[SessionOut] = []
        captured: list[Spark] = []
        unresolved: list[str] = []

        # Apply goal updates in order
        for update in turn.goal_updates:
            if update.action == "create" or goal is None:
                title = update.title or "Untitled goal"
                if goal is not None and update.action == "create":
                    self.goals.pause_others(cid)
                goal = self.goals.create(
                    cid,
                    title=title,
                    domain=update.domain,
                    target=update.target,
                    deadline=update.deadline,
                    baseline=update.baseline,
                    frequency=update.frequency,
                    commitment=update.commitment,
                    status=update.status or "gathering",
                    facts=update.facts or {},
                    pause_others=True,
                )
            else:
                assert goal is not None
                if update.action == "pause_others":
                    self.goals.pause_others(cid, keep_id=goal.id)
                goal = self.goals.apply_update(goal, update)

        # Spark intents without waiting for requested_action
        for intent in turn.intents:
            if intent == "spark_capture":
                content = None
                if turn.requested_action and turn.requested_action.spark_content:
                    content = turn.requested_action.spark_content
                if content:
                    captured.append(self.sparks.capture(content))
            if intent == "spark_dismiss" and turn.requested_action and turn.requested_action.spark_id:
                self.sparks.dismiss(turn.requested_action.spark_id)
            if intent == "spark_promote" and turn.requested_action and turn.requested_action.spark_id:
                if goal:
                    self.sparks.promote(turn.requested_action.spark_id, goal.id)

        action = turn.requested_action or RequestedAction()
        if action.type == "propose_sessions" and goal:
            if goal.status == "gathering" and not (goal.frequency or goal.commitment):
                # Don't propose without a workable cadence — ask instead
                pass
            else:
                _, proposed = propose_for_goal(self.calendar, goal)
                self.goals.save(goal)
        elif action.type == "approve_proposals" and goal:
            batch = action.batch_id or self.calendar.open_proposal_batch(goal.id)
            if batch:
                booked = self.calendar.approve_batch(batch)
        elif action.type == "reject_proposals" and goal:
            batch = action.batch_id or self.calendar.open_proposal_batch(goal.id)
            if batch:
                self.calendar.reject_batch(batch)

        if "session_outcome" in turn.intents and action.session_id and action.outcome:
            self.calendar.mark_outcome(action.session_id, action.outcome)

        reply = turn.assistant_text.strip()
        if proposed and "propose" not in reply.lower() and "session" not in reply.lower():
            reply = reply + "\n\n" + f"Proposed {len(proposed)} session(s) for your approval."

        self._add_message(cid, "assistant", reply)
        return ChatResponse(
            conversation_id=cid,
            reply=reply,
            goal=goal,
            pending_question=turn.clarification,
            proposed_sessions=proposed,
            booked_sessions=booked,
            sparks=captured,
            unresolved=unresolved,
            ai_available=True,
        )

    def get_today(self) -> TodayResponse:
        goals = self.goals.list_open()
        attention: list[str] = []
        pending: list[str] = []
        progress = []
        today = datetime.now().date().isoformat()
        todays = self.calendar.list_sessions(start=today, end=today + "T23:59:59")
        todays = [s for s in todays if s.status in {"scheduled", "proposed", "completed", "missed"}]

        for g in goals:
            counts = self.calendar.progress_for_goal(g.id)
            summary = progress_summary(g, counts)
            progress.append({"goal_id": g.id, "title": g.title, "summary": summary, **counts})
            batch = self.calendar.open_proposal_batch(g.id)
            if batch:
                attention.append(f"{g.title} has sessions waiting for approval")
                pending.append("Approve or reject the proposed sessions?")
            elif g.status == "gathering":
                attention.append(f"{g.title} still needs a clearer plan")
                if not pending:
                    pending.append("What would make this goal realistic this week?")

        spark = None
        open_sparks = self.sparks.list_open()
        if open_sparks:
            # Deterministic resurfacing — no AI call
            spark = open_sparks[0]

        return TodayResponse(
            goals=goals,
            attention=attention,
            pending_questions=pending,
            todays_sessions=[s for s in todays if s.start_at[:10] == today],
            progress=progress,
            resurfaced_spark=spark,
        )
