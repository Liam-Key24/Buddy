"""Buddy control plane — one message entry, one Cloud AI turn, deterministic handlers."""

from __future__ import annotations

import logging
import uuid
from datetime import datetime, timezone
from typing import Any, Protocol

from .ai.groq_provider import GroqError, GroqProvider
from .ai.prompt import SYSTEM_PROMPT, build_user_payload
from .buddy_turn import operations_from_turn, parse_buddy_turn
from .calendar import CalendarService
from .categories import CategoryStore
from .config import Settings, load_settings
from .db import get_connection, init_db
from .goals import GoalStore, progress_summary
from .migrations import run_migrations
from .planning import propose_for_goal, summarize_proposal
from .conversations import ConversationStore
from .folders import FolderStore
from .usage import UsageStore
from .schemas import (
    BuddyTurn,
    CalendarAction,
    ChatResponse,
    ClarificationQuestion,
    Goal,
    GoalPublic,
    GoalUpdate,
    RequestedAction,
    SessionOut,
    Spark,
    TodayNeed,
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
        self.categories = CategoryStore(self.conn)
        self.categories.ensure_defaults()
        self.calendar = CalendarService(self.conn, categories=self.categories)
        self.sparks = SparkService(self.conn)
        self.conversations = ConversationStore(self.conn)
        self.folders = FolderStore(self.conn)
        self.usage = UsageStore(self.conn)
        self._ai = ai
        self._ai_owned = ai is None
        self._call_count = 0  # per-process counter for tests
        self._active_requests: dict[str, Any] = {}

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
            row = self.conversations.get(conversation_id)
            if row and not row.get("deleted_at"):
                return conversation_id
        created = self.conversations.create()
        return created["id"]

    def create_conversation(self) -> dict[str, Any]:
        return self.conversations.create()

    def list_conversations(self) -> list[dict[str, Any]]:
        return self.conversations.list_active()

    def rename_conversation(self, conversation_id: str, title: str) -> dict[str, Any] | None:
        return self.conversations.rename(conversation_id, title)

    def move_conversation(
        self, conversation_id: str, folder_id: str | None
    ) -> dict[str, Any] | None:
        if folder_id and not self.folders.get(folder_id):
            return None
        return self.conversations.move(conversation_id, folder_id)

    def place_conversation(
        self,
        conversation_id: str,
        folder_id: str | None,
        before_id: str | None = None,
    ) -> dict[str, Any] | None:
        if folder_id and not self.folders.get(folder_id):
            return None
        return self.conversations.place(conversation_id, folder_id, before_id)

    def list_folders(self) -> list[dict[str, Any]]:
        return self.folders.list()

    def create_folder(self, title: str) -> dict[str, Any]:
        return self.folders.create(title)

    def rename_folder(self, folder_id: str, title: str) -> dict[str, Any] | None:
        return self.folders.rename(folder_id, title)

    def delete_folder(self, folder_id: str) -> dict[str, Any] | None:
        return self.folders.delete(folder_id)

    def delete_conversation(self, conversation_id: str) -> dict[str, Any] | None:
        row = self.conversations.soft_delete(conversation_id)
        if row:
            # Soft-deleted chats should not keep driving Today Needs you / open goals.
            self.goals.pause_for_conversation(conversation_id)
        return row

    def restore_conversation(self, conversation_id: str) -> dict[str, Any] | None:
        return self.conversations.restore(conversation_id)

    def save_draft(self, conversation_id: str, draft: dict[str, Any]) -> dict[str, Any] | None:
        return self.conversations.save_draft(conversation_id, draft)

    def usage_today(self) -> dict[str, Any]:
        return self.usage.today_summary()

    def _add_message(self, conversation_id: str, role: str, content: str) -> str:
        mid = _new_id()
        self.conn.execute(
            """
            INSERT INTO messages (
                id, conversation_id, role, content, created_at,
                revision_group, revision_of, superseded
            ) VALUES (?, ?, ?, ?, ?, ?, NULL, 0)
            """,
            (mid, conversation_id, role, content, _now(), mid),
        )
        self.conn.execute(
            "UPDATE conversations SET updated_at=? WHERE id=?",
            (_now(), conversation_id),
        )
        self.conn.commit()
        return mid

    def list_messages(self, conversation_id: str) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            """
            SELECT id, conversation_id, role, content, created_at
            FROM messages WHERE conversation_id=? ORDER BY created_at ASC
            """,
            (conversation_id,),
        ).fetchall()
        return [dict(r) for r in rows]

    def get_goal(self, goal_id: str) -> GoalPublic | None:
        goal = self.goals.get(goal_id)
        return self._goal_public(goal) if goal else None

    def list_goals(self) -> list[GoalPublic]:
        return [self._goal_public(g) for g in self.goals.list_managed()]

    def _goal_public(self, goal: Goal) -> GoalPublic:
        counts = self.calendar.progress_for_goal(goal.id)
        span = self.calendar.span_for_goal(goal.id)
        completed = counts.get("completed", 0)
        missed = counts.get("missed", 0)
        started = span.get("started_at") or goal.created_at
        ended = span.get("ended_at")
        if not ended:
            ended = goal.updated_at if goal.status == "done" else goal.deadline
        return GoalPublic(
            **goal.model_dump(),
            events_total=completed + missed,
            events_completed=completed,
            events_missed=missed,
            started_at=started,
            ended_at=ended,
        )

    def remove_goal(self, goal_id: str) -> Goal | None:
        return self.goals.remove(goal_id)

    # --- explicit UI actions (no AI) ---

    def decide_proposal(
        self, batch_id: str, decision: str, *, conversation_id: str | None = None
    ) -> dict[str, Any]:
        if decision == "approve":
            booked = self.calendar.approve_batch(batch_id, conversation_id=conversation_id)
            return {"booked": booked, "undo_batch_id": batch_id if booked else None}
        if decision == "reject":
            n = self.calendar.reject_batch(batch_id)
            return {"rejected": n}
        if decision == "undo":
            restored = self.calendar.undo_batch(batch_id)
            return {"undone": restored}
        return {"ok": False, "detail": "Use chat to describe an adjustment"}

    def mark_outcome(self, session_id: str, outcome: str, notes: str | None = None):
        return self.calendar.mark_outcome(session_id, outcome, notes)

    # --- chat cancel ---

    def cancel_request(self, request_id: str) -> dict[str, Any]:
        entry = self._active_requests.get(request_id)
        if entry is not None:
            entry["cancelled"] = True
        self.usage.mark_cancelled(request_id)
        return {"ok": True, "request_id": request_id, "found": entry is not None}

    def _is_cancelled(self, request_id: str | None) -> bool:
        if not request_id:
            return False
        entry = self._active_requests.get(request_id)
        return bool(entry and entry.get("cancelled"))

    def get_open_proposal(self, conversation_id: str) -> dict[str, Any]:
        """Rebuild proposal card state for a conversation from persisted proposed sessions."""
        goal = self.goals.active_for_conversation(conversation_id)
        if not goal:
            return {"goal": None, "proposed_sessions": [], "proposal_summary": None}
        batch_id = self.calendar.open_proposal_batch(goal.id)
        if not batch_id:
            return {"goal": goal, "proposed_sessions": [], "proposal_summary": None}
        sessions = self.calendar.list_proposed_for_batch(batch_id)
        if not sessions:
            return {"goal": goal, "proposed_sessions": [], "proposal_summary": None}
        summary = summarize_proposal(goal, sessions)
        return {
            "goal": goal,
            "proposed_sessions": sessions,
            "proposal_summary": summary,
        }

    # --- chat entry ---

    def handle_message(
        self,
        message: str,
        conversation_id: str | None = None,
        *,
        request_id: str | None = None,
    ) -> ChatResponse:
        text = message.strip()
        rid = request_id or str(uuid.uuid4())
        self._active_requests[rid] = {"cancelled": False}
        try:
            return self._handle_message_inner(text, conversation_id, request_id=rid)
        finally:
            self._active_requests.pop(rid, None)

    def _handle_message_inner(
        self,
        text: str,
        conversation_id: str | None,
        *,
        request_id: str,
    ) -> ChatResponse:
        cid = self._ensure_conversation(conversation_id)
        self._add_message(cid, "user", text)
        self.conversations.touch_title_from_first_user_message(cid, text)

        # Deterministic short-circuit for explicit approve/reject words when a batch is open.
        goal = self.goals.active_for_conversation(cid)
        if goal:
            batch_id = self.calendar.open_proposal_batch(goal.id)
            lower = text.lower().strip()
            if batch_id and lower in {"approve", "approved", "yes approve", "looks good"}:
                booked = self.calendar.approve_batch(batch_id, conversation_id=cid)
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
                    undo_batch_id=batch_id if booked else None,
                    request_id=request_id,
                    activity=[
                        {"stage": "completed", "label": "Booked after your approval", "detail": "No Cloud AI used"},
                    ],
                )
            if batch_id and lower in {"reject", "reject those", "no thanks", "reject that"}:
                n = self.calendar.reject_batch(batch_id)
                reply = f"Rejected {n} proposed session(s). Say what to change and I'll propose again."
                self._add_message(cid, "assistant", reply)
                return ChatResponse(
                    conversation_id=cid,
                    reply=reply,
                    goal=goal,
                    ai_available=True,
                    request_id=request_id,
                )

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
                request_id=request_id,
            )

        if self._is_cancelled(request_id):
            return ChatResponse(
                conversation_id=cid,
                reply="Stopped.",
                goal=goal,
                ai_available=True,
                request_id=request_id,
                activity=[{"stage": "cancelled", "label": "Stopped", "detail": "No calendar changes"}],
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
                request_id=request_id,
            )

        if self._is_cancelled(request_id):
            self.usage.record(
                conversation_id=cid,
                model=self.settings.groq_model,
                status="cancelled",
                cancelled=True,
                request_id=request_id,
            )
            return ChatResponse(
                conversation_id=cid,
                reply="Stopped.",
                goal=goal,
                ai_available=True,
                request_id=request_id,
                activity=[
                    {
                        "stage": "cancelled",
                        "label": "Stopped",
                        "detail": "Cloud AI response discarded — no calendar changes",
                    }
                ],
            )

        response = self._apply_turn(cid, turn, goal)
        response.request_id = request_id
        return response

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
        calendar_sessions = [
            {
                "id": s.id,
                "title": s.title,
                "start_at": s.start_at,
                "end_at": s.end_at,
                "status": s.status,
                "goal_id": s.goal_id,
            }
            for s in self.calendar.list_sessions()[:40]
        ]
        payload = build_user_payload(
            message=text,
            active_goal=goal.model_dump() if goal else None,
            recent_messages=recent,
            open_proposal_batch_id=batch_id,
            open_sparks=sparks,
            calendar_sessions=calendar_sessions,
        )
        self._call_count += 1
        try:
            raw = self._provider().complete_json(SYSTEM_PROMPT, payload, allow_retry=True)
            stats = getattr(self._provider(), "last_stats", None)
            self.usage.record(
                conversation_id=cid,
                model=self.settings.groq_model,
                status="ok",
                attempt=1 + (1 if stats and getattr(stats, "retried", False) else 0),
                latency_ms=getattr(stats, "latency_ms", None) if stats else None,
                tokens_prompt=getattr(stats, "tokens_prompt", None) if stats else None,
                tokens_completion=getattr(stats, "tokens_completion", None) if stats else None,
                rate_limit=getattr(stats, "rate_limit", None) if stats else None,
                rate_remaining=getattr(stats, "rate_remaining", None) if stats else None,
                rate_reset=getattr(stats, "rate_reset", None) if stats else None,
            )
            return parse_buddy_turn(raw)
        except Exception:
            # One repair retry for malformed structured output (counts as the allowed retry path).
            self._call_count += 1
            repair = (
                payload
                + "\n\nPrevious output was invalid. Return ONLY valid BuddyTurn JSON using exact intent enums."
            )
            try:
                raw2 = self._provider().complete_json(SYSTEM_PROMPT, repair, allow_retry=False)
                stats = getattr(self._provider(), "last_stats", None)
                self.usage.record(
                    conversation_id=cid,
                    model=self.settings.groq_model,
                    status="ok",
                    attempt=2,
                    latency_ms=getattr(stats, "latency_ms", None) if stats else None,
                )
                return parse_buddy_turn(raw2)
            except Exception as exc:  # noqa: BLE001
                self.usage.record(
                    conversation_id=cid,
                    model=self.settings.groq_model,
                    status=getattr(exc, "category", None) or "malformed",
                    attempt=2,
                )
                if isinstance(exc, GroqError):
                    raise
                raise GroqError("malformed", "Cloud AI returned an invalid BuddyTurn") from exc

    @staticmethod
    def _goal_has_cadence(goal: Goal) -> bool:
        facts = goal.facts or {}
        has_plan = isinstance(facts.get("weekly_plan"), dict) and bool(
            (facts.get("weekly_plan") or {}).get("slots")
        )
        return bool(goal.frequency or goal.commitment or has_plan)

    @staticmethod
    def _public_proposal_summary(summary: dict[str, Any] | None) -> dict[str, Any] | None:
        if not summary:
            return None
        return {
            "pattern": summary.get("pattern"),
            "total": summary.get("total"),
            "through": summary.get("through"),
            "text": summary.get("text"),
            "why": summary.get("why") or {},
            "why_lines": summary.get("why_lines") or [],
            "goal_card": summary.get("goal_card") or {},
            "sample": [
                s.model_dump() if hasattr(s, "model_dump") else s
                for s in (summary.get("sample") or [])
            ],
        }

    def _apply_turn(self, cid: str, turn: BuddyTurn, goal: Goal | None) -> ChatResponse:
        proposed: list[SessionOut] = []
        booked: list[SessionOut] = []
        captured: list[Spark] = []
        unresolved: list[str] = []
        deleted_session_ids: list[str] = []
        updated_sessions: list[SessionOut] = []
        operations: list[dict[str, Any]] = []
        touched_goals: list[Goal] = []
        undo_batch_id: str | None = None
        proposal_summary = None

        for op in operations_from_turn(turn):
            try:
                outcome = self._apply_operation(cid, op, goal, touched_goals)
            except Exception as exc:  # noqa: BLE001
                operations.append(
                    {
                        "id": str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "failed",
                        "goal_id": goal.id if goal else None,
                        "detail": exc.__class__.__name__,
                    }
                )
                unresolved.append(op["kind"])
                log.exception("turn_op_failed kind=%s", op["kind"])
                continue

            if outcome.get("goal") is not None:
                goal = outcome["goal"]
                if goal and all(g.id != goal.id for g in touched_goals):
                    touched_goals.append(goal)
            proposed.extend(outcome.get("proposed") or [])
            booked.extend(outcome.get("booked") or [])
            captured.extend(outcome.get("sparks") or [])
            deleted_session_ids.extend(outcome.get("deleted") or [])
            updated_sessions.extend(outcome.get("updated") or [])
            unresolved.extend(outcome.get("errors") or [])
            if outcome.get("proposal_summary"):
                proposal_summary = outcome["proposal_summary"]
                if outcome.get("reply_text"):
                    turn.assistant_text = outcome["reply_text"]
            if outcome.get("undo_batch_id"):
                undo_batch_id = outcome["undo_batch_id"]
            operations.append(
                {
                    "id": str(uuid.uuid4()),
                    "kind": op["kind"],
                    "status": outcome.get("status") or "succeeded",
                    "goal_id": (outcome.get("goal") or goal).id if (outcome.get("goal") or goal) else None,
                    "detail": outcome.get("detail"),
                }
            )

        reply = turn.assistant_text.strip()
        if deleted_session_ids and not proposed:
            n = len(deleted_session_ids)
            suffix = f" Removed {n} calendar session{'s' if n != 1 else ''}."
            if suffix.strip() not in reply:
                reply = f"{reply.rstrip()}{suffix}"
        if updated_sessions and not proposed and not deleted_session_ids:
            n = len(updated_sessions)
            suffix = f" Updated {n} calendar session{'s' if n != 1 else ''}."
            if suffix.strip() not in reply:
                reply = f"{reply.rstrip()}{suffix}"
        if proposed and proposal_summary and proposal_summary.get("text"):
            reply = proposal_summary["text"]

        activity = [
            {"stage": "understanding", "label": "Understanding your goal", "detail": None},
            {"stage": "calendar_read", "label": "Checking calendar availability", "detail": None},
        ]
        if deleted_session_ids or updated_sessions:
            activity.append(
                {
                    "stage": "calendar_write",
                    "label": "Updated your calendar",
                    "detail": f"{len(deleted_session_ids)} removed, {len(updated_sessions)} changed",
                }
            )
            activity.append({"stage": "completed", "label": "Calendar updated", "detail": None})
        elif proposed:
            activity.append(
                {
                    "stage": "proposal_building",
                    "label": "Shaping the proposal",
                    "detail": f"Prepared {len(proposed)} sessions",
                }
            )
            activity.append({"stage": "completed", "label": "Waiting for approval", "detail": "No calendar changes made"})
        elif turn.clarification_questions:
            activity.append({"stage": "answering", "label": "Preparing clarifying questions", "detail": None})
            activity.append({"stage": "completed", "label": "Completed", "detail": "No calendar changes made"})
        else:
            activity.append({"stage": "completed", "label": "Completed", "detail": "No calendar changes made"})

        self._add_message(cid, "assistant", reply)
        return ChatResponse(
            conversation_id=cid,
            reply=reply,
            goal=goal,
            pending_question=turn.clarification if not proposed else "Approve or reject these proposed sessions?",
            proposed_sessions=proposed,
            booked_sessions=booked,
            sparks=captured,
            unresolved=unresolved,
            ai_available=True,
            proposal_summary=proposal_summary,
            clarification_questions=turn.clarification_questions,
            activity=activity,
            undo_batch_id=undo_batch_id,
            deleted_session_ids=deleted_session_ids,
            updated_sessions=updated_sessions,
            operations=operations,
        )

    def _apply_operation(
        self,
        cid: str,
        op: dict[str, Any],
        goal: Goal | None,
        touched_goals: list[Goal],
    ) -> dict[str, Any]:
        kind = op["kind"]
        update: GoalUpdate | None = op.get("goal_update")
        action: RequestedAction | None = op.get("requested_action")
        cal_action: CalendarAction | None = op.get("calendar_action")

        if kind in {"goal_create", "goal_update", "pause_others"} and update is not None:
            if kind == "goal_create" or goal is None:
                created = self.goals.create(
                    cid,
                    title=update.title or "Untitled goal",
                    domain=update.domain,
                    target=update.target,
                    deadline=update.deadline,
                    baseline=update.baseline,
                    frequency=update.frequency,
                    commitment=update.commitment,
                    status=update.status or "gathering",
                    facts=update.facts or {},
                    pause_others=False,
                )
                return {"goal": created, "status": "succeeded", "detail": created.title}
            if kind == "pause_others":
                self.goals.pause_others(cid, keep_id=goal.id)
            updated = self.goals.apply_update(goal, update)
            return {"goal": updated, "status": "succeeded", "detail": updated.title}

        if kind == "spark_capture":
            content = op.get("spark_content")
            if not content:
                return {"status": "failed", "detail": "missing spark content"}
            spark = self.sparks.capture(content)
            return {"sparks": [spark], "status": "succeeded", "detail": spark.content}

        if kind == "spark_dismiss":
            spark_id = op.get("spark_id")
            if spark_id:
                self.sparks.dismiss(spark_id)
            return {"status": "succeeded"}

        if kind == "spark_promote":
            spark_id = op.get("spark_id")
            if spark_id and goal:
                self.sparks.promote(spark_id, goal.id)
            return {"status": "succeeded", "goal": goal}

        if kind == "propose_sessions":
            targets: list[Goal] = []
            for item in [*touched_goals, *([goal] if goal else [])]:
                if item.id not in {g.id for g in targets}:
                    targets.append(item)
            proposed: list[SessionOut] = []
            summary = None
            reply_text = None
            last_goal = goal
            for target in targets:
                if target.status == "gathering" and not self._goal_has_cadence(target):
                    continue
                reply_text, sessions, raw_summary = propose_for_goal(self.calendar, target)
                self.goals.save(target)
                proposed.extend(sessions)
                summary = self._public_proposal_summary(raw_summary)
                last_goal = target
            status = "succeeded" if proposed else "skipped"
            return {
                "goal": last_goal,
                "proposed": proposed,
                "proposal_summary": summary,
                "reply_text": reply_text,
                "status": status,
                "detail": f"{len(proposed)} sessions" if proposed else "no cadence",
            }

        if kind == "approve_proposals" and goal:
            batch = (action.batch_id if action else None) or self.calendar.open_proposal_batch(goal.id)
            booked: list[SessionOut] = []
            undo = None
            if batch:
                booked = self.calendar.approve_batch(batch, conversation_id=cid)
                undo = batch if booked else None
            return {"goal": goal, "booked": booked, "undo_batch_id": undo, "status": "succeeded"}

        if kind == "reject_proposals" and goal:
            batch = (action.batch_id if action else None) or self.calendar.open_proposal_batch(goal.id)
            if batch:
                self.calendar.reject_batch(batch)
            return {"goal": goal, "status": "succeeded"}

        if kind == "session_outcome":
            if action and action.session_id and action.outcome:
                marked = self.calendar.mark_outcome(action.session_id, action.outcome)
                return {"updated": [marked] if marked else [], "status": "succeeded", "goal": goal}
            if cal_action:
                deleted, updated, errors = self.calendar.apply_calendar_action(cal_action)
                return {
                    "deleted": deleted,
                    "updated": updated,
                    "errors": errors,
                    "status": "succeeded" if not errors else "failed",
                    "goal": goal,
                }

        if kind.startswith("calendar_") and cal_action:
            deleted, updated, errors = self.calendar.apply_calendar_action(cal_action)
            return {
                "deleted": deleted,
                "updated": updated,
                "errors": errors,
                "status": "succeeded" if not errors else "failed",
                "goal": goal,
            }

        return {"status": "skipped", "goal": goal}

    def get_today(self) -> TodayResponse:
        goals = self.goals.list_open()
        needs: list[TodayNeed] = []
        progress = []
        today = datetime.now().date().isoformat()
        todays = self.calendar.list_sessions(start=today, end=today + "T23:59:59")
        todays = [s for s in todays if s.status in {"scheduled", "proposed", "completed", "missed"}]

        for g in goals:
            counts = self.calendar.progress_for_goal(g.id)
            summary = progress_summary(g, counts)
            progress.append({"goal_id": g.id, "title": g.title, "summary": summary, **counts})
            conv = self.conversations.get(g.conversation_id)
            if not conv or conv.get("deleted_at"):
                # Chat gone from sidebar — don't surface Needs you for it.
                continue
            batch = self.calendar.open_proposal_batch(g.id)
            if batch:
                needs.append(
                    TodayNeed(
                        id=f"approve-{g.id}",
                        kind="approve",
                        title=f"{g.title} has sessions waiting for approval",
                        detail="Approve, reject, or adjust the proposed plan",
                        goal_id=g.id,
                        conversation_id=g.conversation_id,
                        proposal_batch_id=batch,
                    )
                )
            elif g.status == "gathering":
                needs.append(
                    TodayNeed(
                        id=f"gather-{g.id}",
                        kind="gathering",
                        title=f"{g.title} still needs a clearer plan",
                        detail="What would make this goal realistic this week?",
                        goal_id=g.id,
                        conversation_id=g.conversation_id,
                    )
                )

        spark = None
        open_sparks = self.sparks.list_open()
        if open_sparks:
            # Deterministic resurfacing — no AI call
            spark = open_sparks[0]

        # Legacy string lists kept for older clients; count should use `needs`.
        attention = [n.title for n in needs]
        pending_questions = [n.detail for n in needs if n.detail]

        return TodayResponse(
            goals=goals,
            attention=attention,
            pending_questions=pending_questions,
            needs=needs,
            todays_sessions=[s for s in todays if s.start_at[:10] == today],
            progress=progress,
            resurfaced_spark=spark,
        )
