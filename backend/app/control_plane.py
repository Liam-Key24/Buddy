"""Buddy control plane — one message entry, one Cloud AI turn, deterministic handlers."""

from __future__ import annotations

import json
import logging
import threading
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, Protocol

from pydantic import ValidationError

from .ai.groq_provider import GroqError, GroqProvider
from .ai.prompt import SYSTEM_PROMPT, build_user_payload
from .buddy_turn import operations_from_turn, parse_buddy_turn
from .calendar import CalendarService
from .categories import CategoryStore
from .config import Settings, load_settings
from .db import get_connection, init_db
from .goals import GoalStore, progress_summary
from .migrations import run_migrations
from .mutation_policy import classify_operation, preview_payload
from .planning import propose_for_goal, summarize_proposal
from .conversations import ConversationStore
from .folders import FolderStore
from .timeutil import DEFAULT_TIMEZONE, local_now, local_today
from .turns import TurnStore
from .usage import UsageStore
from .schemas import (
    BuddyTurn,
    CalendarAction,
    ChatResponse,
    ClarificationAnswer,
    ClarificationQuestion,
    Goal,
    GoalPublic,
    GoalUpdate,
    KNOWN_REQUESTED_ACTIONS,
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
        self.turns = TurnStore(self.conn)
        self._ai = ai
        self._ai_owned = ai is None
        self._call_count = 0  # per-process counter for tests
        self._active_requests: dict[str, Any] = {}
        self._turn_lock = threading.Lock()
        self._conv_locks: dict[str, threading.Lock] = {}
        self._conv_locks_guard = threading.Lock()

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

    def _lock_for_conversation(self, conversation_id: str) -> threading.Lock:
        with self._conv_locks_guard:
            lock = self._conv_locks.get(conversation_id)
            if lock is None:
                lock = threading.Lock()
                self._conv_locks[conversation_id] = lock
            return lock

    def timezone_name(self) -> str:
        row = self.conn.execute(
            "SELECT value FROM buddy_meta WHERE key='timezone'"
        ).fetchone()
        if not row:
            return DEFAULT_TIMEZONE
        value = row["value"] if hasattr(row, "keys") else row[0]
        return value or DEFAULT_TIMEZONE

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

    def revert_to(self, conversation_id: str, message_id: str) -> dict[str, Any]:
        row = self.conn.execute(
            "SELECT * FROM messages WHERE id=? AND conversation_id=?",
            (message_id, conversation_id),
        ).fetchone()
        if not row:
            return {"ok": False, "reverted": 0}
        self.conn.execute(
            """
            UPDATE messages SET superseded=1
            WHERE conversation_id=? AND superseded=0 AND created_at>?
            """,
            (conversation_id, row["created_at"]),
        )
        changed = self.conn.execute("SELECT changes() AS n").fetchone()
        self.conn.commit()
        return {"ok": True, "reverted": int(changed["n"] if changed else 0)}

    def save_draft(self, conversation_id: str, draft: dict[str, Any]) -> dict[str, Any] | None:
        return self.conversations.save_draft(conversation_id, draft)

    def usage_today(self) -> dict[str, Any]:
        return self.usage.today_summary()

    def _add_message(
        self,
        conversation_id: str,
        role: str,
        content: str,
        *,
        revision_group: str | None = None,
        revision_of: str | None = None,
        superseded: int = 0,
    ) -> str:
        mid = _new_id()
        self.conn.execute(
            """
            INSERT INTO messages (
                id, conversation_id, role, content, created_at,
                revision_group, revision_of, superseded
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                mid,
                conversation_id,
                role,
                content,
                _now(),
                revision_group or mid,
                revision_of,
                superseded,
            ),
        )
        self.conn.execute(
            "UPDATE conversations SET updated_at=? WHERE id=?",
            (_now(), conversation_id),
        )
        self.conn.commit()
        return mid

    def _supersede_revision_path(self, conversation_id: str, message_id: str) -> str:
        row = self.conn.execute(
            "SELECT * FROM messages WHERE id=? AND conversation_id=?",
            (message_id, conversation_id),
        ).fetchone()
        if not row:
            return message_id
        group = row["revision_group"] or message_id
        self.conn.execute(
            """
            UPDATE messages SET superseded=1
            WHERE conversation_id=? AND revision_group=? AND superseded=0
              AND created_at >= ?
            """,
            (conversation_id, group, row["created_at"]),
        )
        # Unapproved proposals from the old revision are dropped.
        goal = self.goals.active_for_conversation(conversation_id)
        if goal:
            batch = self.calendar.open_proposal_batch(goal.id)
            if batch:
                self.calendar.reject_batch(batch)
        self.conn.commit()
        return group

    def list_messages(self, conversation_id: str, *, include_superseded: bool = False) -> list[dict[str, Any]]:
        sql = """
            SELECT id, conversation_id, role, content, created_at,
                   revision_group, revision_of, superseded
            FROM messages WHERE conversation_id=?
        """
        if not include_superseded:
            sql += " AND superseded=0"
        sql += " ORDER BY created_at ASC"
        rows = self.conn.execute(sql, (conversation_id,)).fetchall()
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
        committed = False
        if entry is not None:
            entry["cancelled"] = True
            client = entry.get("http_client")
            if client is not None:
                try:
                    client.close()
                except Exception:
                    pass
        turn = self.turns.get(request_id)
        status = turn["status"] if turn else None
        if status in {"committing", "completed"}:
            committed = True
        elif turn and status not in {"cancelled"}:
            self.turns.update(request_id, status="cancelled")
        self.usage.mark_cancelled(request_id)
        return {
            "ok": True,
            "request_id": request_id,
            "found": entry is not None or turn is not None,
            "turn_status": status or "unknown",
            "committed": committed,
        }

    def _is_cancelled(self, request_id: str | None) -> bool:
        if not request_id:
            return False
        entry = self._active_requests.get(request_id)
        if entry and entry.get("cancelled"):
            return True
        turn = self.turns.get(request_id)
        return bool(turn and turn.get("status") == "cancelled")

    def get_open_proposal(self, conversation_id: str) -> dict[str, Any]:
        """Rebuild proposal card state for a conversation from persisted proposed sessions."""
        goal = self.goals.active_for_conversation(conversation_id)
        pending = self.turns.latest_awaiting_approval(conversation_id)
        extra: dict[str, Any] = {
            "mutation_preview": None,
            "clarification_questions": [],
        }
        if pending:
            extra["mutation_preview"] = {
                "operations": pending.get("pending_ops") or [],
            }
            extra["clarification_questions"] = pending.get("clarifications") or []
        conv = self.conversations.get(conversation_id)
        draft = (conv or {}).get("draft") or {}
        if draft.get("clarification_questions") and not extra["clarification_questions"]:
            extra["clarification_questions"] = draft["clarification_questions"]
        if not goal:
            return {"goal": None, "proposed_sessions": [], "proposal_summary": None, **extra}
        batch_id = self.calendar.open_proposal_batch(goal.id)
        if not batch_id:
            return {"goal": goal, "proposed_sessions": [], "proposal_summary": None, **extra}
        sessions = self.calendar.list_proposed_for_batch(batch_id)
        if not sessions:
            return {"goal": goal, "proposed_sessions": [], "proposal_summary": None, **extra}
        summary = summarize_proposal(goal, sessions)
        return {
            "goal": goal,
            "proposed_sessions": sessions,
            "proposal_summary": summary,
            **extra,
        }

    # --- chat entry ---

    def handle_message(
        self,
        message: str,
        conversation_id: str | None = None,
        *,
        request_id: str | None = None,
        clarification_answers: list[ClarificationAnswer] | list[dict] | None = None,
        revision_of: str | None = None,
    ) -> ChatResponse:
        text = message.strip()
        rid = request_id or str(uuid.uuid4())
        existing = self.turns.get(rid)
        if existing and existing.get("response"):
            return ChatResponse.model_validate(existing["response"])
        self._active_requests[rid] = {"cancelled": False, "http_client": None}
        try:
            cid = (existing or {}).get("conversation_id") or self._ensure_conversation(conversation_id)
            with self._lock_for_conversation(cid):
                return self._handle_message_inner(
                    text,
                    cid,
                    request_id=rid,
                    clarification_answers=clarification_answers or [],
                    revision_of=revision_of,
                )
        finally:
            self._active_requests.pop(rid, None)

    def _handle_message_inner(
        self,
        text: str,
        cid: str,
        *,
        request_id: str,
        clarification_answers: list,
        revision_of: str | None,
    ) -> ChatResponse:
        existing = self.turns.get(request_id)
        if existing and existing.get("response"):
            return ChatResponse.model_validate(existing["response"])

        begun = existing or self.turns.try_begin(request_id, cid, text)
        if begun is None:
            stored = self.turns.get(request_id)
            if stored and stored.get("response"):
                return ChatResponse.model_validate(stored["response"])
            begun = stored

        answers = []
        for item in clarification_answers:
            if hasattr(item, "model_dump"):
                answers.append(item.model_dump())
            elif isinstance(item, dict):
                answers.append(item)

        revision_group = None
        if revision_of:
            revision_group = self._supersede_revision_path(cid, revision_of)

        user_mid = None
        if not (begun and begun.get("user_message_id")):
            user_mid = self._add_message(
                cid,
                "user",
                text,
                revision_group=revision_group,
                revision_of=revision_of,
            )
            self.turns.update(request_id, user_message_id=user_mid, revision_group=revision_group)
        self.conversations.touch_title_from_first_user_message(cid, text)

        goal = self.goals.active_for_conversation(cid)
        lower = text.lower().strip()
        approve_words = {"approve", "approved", "yes approve", "looks good"}
        reject_words = {"reject", "reject those", "no thanks", "reject that", "cancel"}

        pending = self.turns.latest_awaiting_approval(cid)
        if pending and lower in approve_words:
            response = self._commit_pending_turn(cid, pending, goal, request_id)
            return self._finish_response(request_id, cid, response)
        if pending and lower in reject_words:
            self.turns.update(pending["request_id"], status="cancelled", pending_ops_json=[])
            reply = "Cancelled that preview. Nothing on the calendar changed."
            self._add_message(cid, "assistant", reply)
            response = ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                ai_available=True,
                request_id=request_id,
                activity=[{"stage": "cancelled", "label": "Preview cancelled", "detail": "No calendar changes"}],
            )
            return self._finish_response(request_id, cid, response)

        if goal:
            batch_id = self.calendar.open_proposal_batch(goal.id)
            if batch_id and lower in approve_words:
                booked = self.calendar.approve_batch(batch_id, conversation_id=cid)
                reply = (
                    f"Booked {len(booked)} session(s) once each and linked them to {goal.title}. "
                    "Tell me when one is completed or missed and I'll record it."
                )
                self._add_message(cid, "assistant", reply)
                response = ChatResponse(
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
                return self._finish_response(request_id, cid, response)
            if batch_id and lower in reject_words:
                n = self.calendar.reject_batch(batch_id)
                reply = f"Rejected {n} proposed session(s). Say what to change and I'll propose again."
                self._add_message(cid, "assistant", reply)
                response = ChatResponse(
                    conversation_id=cid,
                    reply=reply,
                    goal=goal,
                    ai_available=True,
                    request_id=request_id,
                )
                return self._finish_response(request_id, cid, response)

        if not self.settings.groq_configured and self._ai is None:
            reply = (
                "Today, Calendar and Sparks still work offline. "
                "Conversational understanding needs Cloud AI — add GROQ_API_KEY "
                "(and enable Zero Data Retention in the Groq console) to continue planning in Chat."
            )
            self._add_message(cid, "assistant", reply)
            response = ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                ai_available=False,
                request_id=request_id,
            )
            return self._finish_response(request_id, cid, response)

        if self._is_cancelled(request_id):
            response = ChatResponse(
                conversation_id=cid,
                reply="Stopped.",
                goal=goal,
                ai_available=True,
                request_id=request_id,
                stopped=True,
                stop_committed=False,
                activity=[{"stage": "cancelled", "label": "Stopped", "detail": "No calendar changes"}],
            )
            return self._finish_response(request_id, cid, response, status="cancelled")

        self.turns.update(request_id, status="interpreting")
        try:
            turn, context_categories = self._run_ai_turn(
                cid, text, goal, request_id=request_id, answers=answers
            )
        except GroqError as exc:
            if exc.category == "cancelled" or self._is_cancelled(request_id):
                response = ChatResponse(
                    conversation_id=cid,
                    reply="Stopped.",
                    goal=goal,
                    ai_available=True,
                    request_id=request_id,
                    stopped=True,
                    stop_committed=False,
                    activity=[{"stage": "cancelled", "label": "Stopped", "detail": "No calendar changes"}],
                )
                return self._finish_response(request_id, cid, response, status="cancelled")
            reply = (
                "I couldn't reach Cloud AI just now, so I didn't change your plan. "
                f"({exc.category.replace('_', ' ')}) "
                "Today, Calendar and Sparks remain available."
            )
            self._add_message(cid, "assistant", reply)
            response = ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                ai_available=False,
                request_id=request_id,
                activity=[{"stage": "failed", "label": "Cloud AI unavailable", "detail": "No calendar changes"}],
            )
            return self._finish_response(request_id, cid, response, status="failed")

        if self._is_cancelled(request_id):
            self.usage.record(
                conversation_id=cid,
                model=self.settings.groq_model,
                status="cancelled",
                cancelled=True,
            )
            response = ChatResponse(
                conversation_id=cid,
                reply="Stopped.",
                goal=goal,
                ai_available=True,
                request_id=request_id,
                stopped=True,
                stop_committed=False,
                activity=[
                    {
                        "stage": "cancelled",
                        "label": "Stopped",
                        "detail": "Cloud AI response discarded — no calendar changes",
                    }
                ],
            )
            return self._finish_response(request_id, cid, response, status="cancelled")

        self.turns.update(
            request_id,
            status="committing",
            context_categories_json=context_categories,
        )
        response = self._apply_turn(
            cid, turn, goal, request_id=request_id, context_categories=context_categories
        )
        response.request_id = request_id
        status = "awaiting_approval" if response.mutation_preview else "completed"
        return self._finish_response(request_id, cid, response, status=status)

    def _finish_response(
        self,
        request_id: str,
        cid: str,
        response: ChatResponse,
        *,
        status: str = "completed",
    ) -> ChatResponse:
        response.request_id = request_id
        payload = response.model_dump(mode="json")
        self.turns.update(request_id, status=status, response_json=payload)
        return response

    def _commit_pending_turn(
        self,
        cid: str,
        pending: dict[str, Any],
        goal: Goal | None,
        request_id: str,
    ) -> ChatResponse:
        ops = pending.get("pending_ops") or []
        deleted: list[str] = []
        updated: list[SessionOut] = []
        unresolved: list[str] = []
        for raw in ops:
            if raw.get("kind") == "pause_others" and goal:
                self.goals.pause_others(cid, keep_id=goal.id)
                continue
            action_data = raw.get("calendar_action") or raw
            try:
                action = CalendarAction.model_validate(action_data)
            except ValidationError:
                unresolved.append("invalid_preview")
                continue
            d, u, errors = self.calendar.apply_calendar_action(action)
            deleted.extend(d)
            updated.extend(u)
            unresolved.extend(errors)
        self.turns.update(pending["request_id"], status="completed", pending_ops_json=[])
        n = len(deleted) + len(updated)
        reply = f"Applied {n} calendar change{'s' if n != 1 else ''}."
        if unresolved:
            reply += " Some items could not be matched."
        self._add_message(cid, "assistant", reply)
        if deleted or updated:
            notify_detail = f"{len(deleted)} removed, {len(updated)} changed"
        else:
            notify_detail = "No matching sessions"
        return ChatResponse(
            conversation_id=cid,
            reply=reply,
            goal=goal,
            ai_available=True,
            request_id=request_id,
            deleted_session_ids=deleted,
            updated_sessions=updated,
            unresolved=unresolved,
            activity=[
                {"stage": "committing", "label": "Applying approved changes", "detail": notify_detail},
                {"stage": "completed", "label": "Calendar updated", "detail": None},
            ],
        )

    def _run_ai_turn(
        self,
        cid: str,
        text: str,
        goal: Goal | None,
        *,
        request_id: str,
        answers: list[dict] | None = None,
    ) -> tuple[BuddyTurn, list[str]]:
        user_mid = None
        turn_row = self.turns.get(request_id)
        if turn_row:
            user_mid = turn_row.get("user_message_id")
        recent = []
        size = 0
        for m in reversed(self.list_messages(cid)):
            if user_mid and m.get("id") == user_mid:
                continue
            if m.get("content") == text and m.get("role") == "user" and not recent:
                continue
            piece = {"role": m["role"], "content": m["content"]}
            size += len(m["content"] or "")
            if len(recent) >= 8 or size > 4000:
                break
            recent.append(piece)
        recent.reverse()

        categories: list[str] = ["history"]
        wants_calendar = self._needs_calendar_context(text)
        wants_sparks = self._needs_spark_context(text)
        batch_id = self.calendar.open_proposal_batch(goal.id) if goal else None
        sparks = []
        if wants_sparks:
            sparks = [{"id": s.id, "content": s.content} for s in self.sparks.list_open()[:8]]
            categories.append("sparks")
        calendar_sessions = []
        if wants_calendar:
            categories.append("calendar")
            tz_name = self.timezone_name()
            now = local_now(tz_name)
            start = (now - timedelta(days=7)).date().isoformat()
            end = (now + timedelta(days=21)).date().isoformat() + "T23:59:59"
            ranked = []
            for s in self.calendar.list_sessions(start=start, end=end):
                ranked.append(
                    {
                        "id": s.id,
                        "title": s.title,
                        "start_at": s.start_at,
                        "end_at": s.end_at,
                        "status": s.status,
                        "goal_id": s.goal_id,
                    }
                )
            calendar_sessions = ranked[:20]
        if goal:
            categories.append("active_goal")
        open_goals = [g.model_dump() for g in self.goals.list_open() if g.conversation_id == cid][:6]
        if open_goals:
            categories.append("goals")
        log.info("chat_context categories=%s", ",".join(categories))

        tz_name = self.timezone_name()
        now = local_now(tz_name)
        payload = build_user_payload(
            message=text,
            active_goal=goal.model_dump() if goal else None,
            recent_messages=recent,
            open_proposal_batch_id=batch_id,
            open_sparks=sparks,
            calendar_sessions=calendar_sessions,
            timezone=tz_name,
            local_now=now.isoformat(),
            today=local_today(tz_name, at=now),
            open_goals=open_goals,
            clarification_answers=answers,
        )
        provider = self._provider()
        entry = self._active_requests.get(request_id)
        if entry is not None and hasattr(provider, "_http"):
            try:
                entry["http_client"] = provider._http()
            except Exception:
                pass

        def _cancelled() -> bool:
            return self._is_cancelled(request_id)

        def _record(status: str, *, attempt: int, stats=None, cancelled: bool = False) -> None:
            self.usage.record(
                conversation_id=cid,
                model=self.settings.groq_model,
                status=status,
                attempt=attempt,
                cancelled=cancelled,
                latency_ms=getattr(stats, "latency_ms", None) if stats else None,
                tokens_prompt=getattr(stats, "tokens_prompt", None) if stats else None,
                tokens_completion=getattr(stats, "tokens_completion", None) if stats else None,
                rate_limit=getattr(stats, "rate_limit", None) if stats else None,
                rate_remaining=getattr(stats, "rate_remaining", None) if stats else None,
                rate_reset=getattr(stats, "rate_reset", None) if stats else None,
            )

        self._call_count += 1
        try:
            kwargs: dict[str, Any] = {"allow_retry": False}
            try:
                raw = provider.complete_json(
                    SYSTEM_PROMPT, payload, cancel_check=_cancelled, **kwargs
                )
            except TypeError:
                raw = provider.complete_json(SYSTEM_PROMPT, payload, allow_retry=False)
            try:
                parsed = parse_buddy_turn(raw)
            except (ValidationError, ValueError) as parse_exc:
                raise GroqError("malformed", "Cloud AI returned an invalid BuddyTurn") from parse_exc
            stats = getattr(provider, "last_stats", None)
            _record("ok", attempt=1, stats=stats)
            return parsed, categories
        except GroqError as exc:
            if exc.category != "malformed":
                _record(exc.category, attempt=1, stats=getattr(provider, "last_stats", None))
                raise
            self._call_count += 1
            repair = (
                payload
                + "\n\nPrevious output was invalid. Return ONLY valid BuddyTurn JSON using exact intent enums."
            )
            try:
                try:
                    raw2 = provider.complete_json(
                        SYSTEM_PROMPT, repair, cancel_check=_cancelled, allow_retry=False
                    )
                except TypeError:
                    raw2 = provider.complete_json(SYSTEM_PROMPT, repair, allow_retry=False)
                stats = getattr(provider, "last_stats", None)
                _record("ok", attempt=2, stats=stats)
                return parse_buddy_turn(raw2), categories
            except Exception as repair_exc:  # noqa: BLE001
                _record(
                    getattr(repair_exc, "category", None) or "malformed",
                    attempt=2,
                )
                if isinstance(repair_exc, GroqError):
                    raise
                raise GroqError("malformed", "Cloud AI returned an invalid BuddyTurn") from repair_exc
        except ValidationError:
            self._call_count += 1
            repair = (
                payload
                + "\n\nPrevious output was invalid. Return ONLY valid BuddyTurn JSON using exact intent enums."
            )
            try:
                raw2 = provider.complete_json(SYSTEM_PROMPT, repair, allow_retry=False)
            except TypeError:
                raw2 = provider.complete_json(SYSTEM_PROMPT, repair, allow_retry=False)
            stats = getattr(provider, "last_stats", None)
            _record("ok", attempt=2, stats=stats)
            return parse_buddy_turn(raw2), categories

    @staticmethod
    def _needs_calendar_context(text: str) -> bool:
        lower = text.lower()
        hints = (
            "calendar",
            "session",
            "delete",
            "remove",
            "move",
            "reschedule",
            "rename",
            "propose",
            "schedule",
            "look at",
            "monday",
            "tuesday",
            "wednesday",
            "thursday",
            "friday",
            "saturday",
            "sunday",
            "tonight",
            "tomorrow",
            "weekend",
            "completed",
            "missed",
        )
        return any(h in lower for h in hints)

    @staticmethod
    def _needs_spark_context(text: str) -> bool:
        lower = text.lower()
        return "spark" in lower or lower.startswith("idea:") or "standing desk" in lower

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

    def _apply_turn(
        self,
        cid: str,
        turn: BuddyTurn,
        goal: Goal | None,
        *,
        request_id: str | None = None,
        context_categories: list[str] | None = None,
    ) -> ChatResponse:
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
        pending_ops: list[dict[str, Any]] = []
        preview_records: list[dict[str, Any]] = []
        questions = list(turn.clarification_questions)

        action = turn.requested_action
        if action and action.type not in KNOWN_REQUESTED_ACTIONS:
            operations.append(
                {
                    "id": str(uuid.uuid4()),
                    "kind": action.type,
                    "status": "failed",
                    "goal_id": goal.id if goal else None,
                    "detail": f"unknown action {action.type}",
                }
            )
            unresolved.append(f"requested_action.type: unknown action {action.type}")

        for err in turn.validation_errors:
            path = err.get("path") or "turn"
            detail = err.get("error") or "invalid"
            unresolved.append(f"{path}: {detail}")
            operations.append(
                {
                    "id": str(uuid.uuid4()),
                    "kind": "validation",
                    "status": "failed",
                    "goal_id": goal.id if goal else None,
                    "detail": f"{path}: {detail}",
                }
            )

        for op in operations_from_turn(turn):
            cal_action: CalendarAction | None = op.get("calendar_action")
            targets = self.calendar.resolve_action_targets(cal_action) if cal_action else []
            policy = classify_operation(
                kind=op["kind"],
                target_count=len(targets),
                all_matching=bool(cal_action.all_matching) if cal_action else False,
                has_session_id=bool(cal_action and cal_action.session_id) or bool(
                    op.get("requested_action") and getattr(op.get("requested_action"), "session_id", None)
                ),
                confidence=turn.confidence,
            )
            if policy == "clarify":
                operations.append(
                    {
                        "id": str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "needs_clarification",
                        "goal_id": goal.id if goal else None,
                        "detail": "Need a specific session to change",
                    }
                )
                if not questions:
                    questions.append(
                        ClarificationQuestion(
                            id="which-session",
                            label="Which calendar session should I change?",
                            answer_type="short_text",
                        )
                    )
                continue
            if policy == "preview":
                records = [
                    {
                        "id": t.id,
                        "title": t.title,
                        "start_at": t.start_at,
                        "end_at": t.end_at,
                        "status": t.status,
                    }
                    for t in targets
                ]
                payload = preview_payload(
                    kind=op["kind"],
                    records=records,
                    why="Destructive or bulk calendar changes wait for your approval.",
                )
                pending_ops.append(
                    {
                        "kind": op["kind"],
                        "calendar_action": cal_action.model_dump() if cal_action else None,
                        "preview": payload,
                    }
                )
                preview_records.extend(records)
                operations.append(
                    {
                        "id": str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "needs_approval",
                        "goal_id": goal.id if goal else None,
                        "detail": payload["why"],
                        "preview": payload,
                    }
                )
                continue

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

        reply = (turn.assistant_text or "").strip()
        if pending_ops:
            n = len(preview_records)
            suffix = f" Approve to apply {n} calendar change{'s' if n != 1 else ''}."
            if suffix.strip() not in reply:
                reply = f"{reply.rstrip()}{suffix}" if reply else suffix.strip()
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
        if proposed and not reply:
            total = proposal_summary.get("total") if proposal_summary else len(proposed)
            title = goal.title if goal else "your goal"
            reply = f"Proposed {total} session(s) for {title}. Nothing is booked until you approve."

        cats = set(context_categories or [])
        activity: list[dict[str, Any]] = [
            {"stage": "receiving", "label": "Received your message", "detail": None},
            {"stage": "interpreting", "label": "Understanding your goal", "detail": None},
        ]
        if "calendar" in cats:
            activity.insert(
                1,
                {"stage": "context", "label": "Checking calendar availability", "detail": None},
            )
        elif cats:
            activity.insert(
                1,
                {"stage": "context", "label": "Loading relevant context", "detail": ",".join(sorted(cats))},
            )
        mutation_preview = None
        if pending_ops:
            mutation_preview = preview_payload(
                kind="calendar",
                records=preview_records,
                why="Waiting for approval before changing the calendar.",
            )
            activity.append(
                {"stage": "preview", "label": "Waiting for approval", "detail": f"{len(preview_records)} change(s)"}
            )
            activity.append({"stage": "awaiting_approval", "label": "Approve to apply", "detail": None})
        elif deleted_session_ids or updated_sessions:
            activity.append(
                {
                    "stage": "committing",
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
        elif questions:
            activity.append({"stage": "clarifying", "label": "Preparing clarifying questions", "detail": None})
            activity.append({"stage": "completed", "label": "Completed", "detail": "No calendar changes made"})
        else:
            activity.append({"stage": "completed", "label": "Completed", "detail": "No calendar changes made"})

        failed_only = operations and all(
            o.get("status") in {"failed", "needs_clarification"} for o in operations
        )
        if failed_only and not proposed and not captured and not pending_ops:
            reply = "I couldn't apply that — the action wasn't valid."

        if request_id and pending_ops:
            self.turns.update(
                request_id,
                pending_ops_json=pending_ops,
                clarifications_json=[q.model_dump() for q in questions],
            )
        elif request_id and questions:
            self.turns.update(request_id, clarifications_json=[q.model_dump() for q in questions])
            conv = self.conversations.get(cid) or {}
            draft = dict(conv.get("draft") or {})
            draft["clarification_questions"] = [q.model_dump() for q in questions]
            self.conversations.save_draft(cid, draft)

        self._add_message(cid, "assistant", reply)
        pending_q = None
        if pending_ops:
            pending_q = "Approve or cancel these calendar changes?"
        elif proposed:
            pending_q = "Approve or reject these proposed sessions?"
        else:
            pending_q = turn.clarification
        return ChatResponse(
            conversation_id=cid,
            reply=reply,
            goal=goal,
            pending_question=pending_q,
            proposed_sessions=proposed,
            booked_sessions=booked,
            sparks=captured,
            unresolved=unresolved,
            ai_available=True,
            proposal_summary=proposal_summary,
            clarification_questions=questions,
            activity=activity,
            undo_batch_id=undo_batch_id,
            deleted_session_ids=deleted_session_ids,
            updated_sessions=updated_sessions,
            operations=operations,
            mutation_preview=mutation_preview,
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
