"""Buddy control plane — one message entry, one Cloud AI turn, deterministic handlers."""

from __future__ import annotations

import json
import logging
import re
import threading
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any, Protocol

from pydantic import ValidationError

from .ai.groq_provider import GroqError, GroqProvider
from .ai.prompt import SYSTEM_PROMPT, build_user_payload
from .buddy_turn import (
    operation_calendar_action,
    operation_goal_update,
    operations_from_turn,
    parse_buddy_turn,
)
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
    KNOWN_REQUESTED_ACTIONS,
    ProposalGroup,
    SessionOut,
    Spark,
    TodayNeed,
    TodayResponse,
)
from .sparks import SparkService

log = logging.getLogger("buddy.control_plane")


class RevertBlocked(Exception):
    def __init__(self, detail: str):
        super().__init__(detail)
        self.detail = detail


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

    def revert_to(
        self,
        conversation_id: str,
        message_id: str,
        *,
        include_target: bool = False,
    ) -> dict[str, Any]:
        row = self.conn.execute(
            "SELECT * FROM messages WHERE id=? AND conversation_id=?",
            (message_id, conversation_id),
        ).fetchone()
        if not row:
            return {"ok": False, "reverted": 0}
        cutoff = row["created_at"]
        revision_group = row["revision_group"] or message_id
        blocked = self._revert_block_reason(conversation_id, cutoff)
        if blocked:
            return {
                "ok": False,
                "blocked": True,
                "detail": blocked,
                "reverted": 0,
                "revision_group": revision_group,
            }

        later_turns = self.turns.list_since(
            conversation_id,
            created_at=cutoff,
            include_user_message_id=message_id if include_target else None,
        )
        for turn in later_turns:
            self._undo_effects(turn.get("effects") or {})
            if turn.get("status") == "awaiting_approval":
                self.turns.update(turn["request_id"], status="cancelled", pending_ops_json=[])

        self._undo_approvals_since(conversation_id, cutoff)

        self.conn.execute(
            """
            UPDATE messages SET superseded=1
            WHERE conversation_id=? AND superseded=0 AND created_at>?
            """,
            (conversation_id, cutoff),
        )
        later = self.conn.execute("SELECT changes() AS n").fetchone()
        if include_target:
            self.conn.execute(
                "UPDATE messages SET superseded=1 WHERE id=? AND conversation_id=?",
                (message_id, conversation_id),
            )
        self.conn.commit()
        return {
            "ok": True,
            "reverted": int(later["n"] if later else 0) + (1 if include_target else 0),
            "revision_group": revision_group,
        }

    def _revert_block_reason(self, conversation_id: str, cutoff: str) -> str | None:
        later = self.turns.list_since(conversation_id, created_at=cutoff)
        session_ids: set[str] = set()
        for turn in later:
            session_ids.update(self._session_ids_touched_by_turn(turn))
        events = self.conn.execute(
            """
            SELECT session_ids_json FROM approval_events
            WHERE conversation_id=? AND approved_at>? AND undone_at IS NULL
            """,
            (conversation_id, cutoff),
        ).fetchall()
        for event in events:
            try:
                session_ids.update(json.loads(event["session_ids_json"] or "[]"))
            except json.JSONDecodeError:
                continue
        for sid in session_ids:
            session = self.calendar.get_session(sid)
            if session and session.status in {"completed", "missed"}:
                return (
                    "Can't edit that — a later calendar session was already marked "
                    f"{session.status}. Undo or keep those records first."
                )
        return None

    def _session_ids_touched_by_turn(self, turn: dict[str, Any]) -> set[str]:
        ids: set[str] = set()
        effects = turn.get("effects") or {}
        ids.update(effects.get("sessions_created") or [])
        ids.update((effects.get("sessions_before") or {}).keys())
        for snap in effects.get("sessions_deleted") or []:
            if isinstance(snap, dict) and snap.get("id"):
                ids.add(str(snap["id"]))
        for key in ("batches_approved", "batches_proposed", "batches_rejected"):
            for batch in effects.get(key) or []:
                rows = self.conn.execute(
                    "SELECT id FROM sessions WHERE proposal_batch_id=?",
                    (batch,),
                ).fetchall()
                for row in rows:
                    ids.add(row["id"])
        resp = turn.get("response") or {}
        for key in ("booked_sessions", "proposed_sessions", "updated_sessions"):
            for session in resp.get(key) or []:
                if isinstance(session, dict) and session.get("id"):
                    ids.add(str(session["id"]))
        ids.update(str(sid) for sid in (resp.get("deleted_session_ids") or []) if sid)
        return ids

    def _undo_approvals_since(self, conversation_id: str, cutoff: str) -> None:
        events = self.conn.execute(
            """
            SELECT batch_id FROM approval_events
            WHERE conversation_id=? AND approved_at>? AND undone_at IS NULL
            ORDER BY approved_at DESC
            """,
            (conversation_id, cutoff),
        ).fetchall()
        for event in events:
            self.calendar.undo_batch(event["batch_id"])

    def _empty_effects(self) -> dict[str, Any]:
        return {
            "goals_created": [],
            "goals_before": {},
            "sessions_created": [],
            "sessions_deleted": [],
            "sessions_before": {},
            "batches_proposed": [],
            "batches_approved": [],
            "batches_rejected": [],
            "sparks_created": [],
        }

    def _record_goal_before(self, effects: dict[str, Any], goal: Goal | None) -> None:
        if not goal or goal.id in effects["goals_before"] or goal.id in effects["goals_created"]:
            return
        effects["goals_before"][goal.id] = goal.model_dump(mode="json")

    def _snapshot_session(
        self,
        effects: dict[str, Any],
        session: SessionOut | None,
        *,
        deleted: bool = False,
    ) -> None:
        if not session:
            return
        snap = self.calendar.session_snapshot(session)
        effects["sessions_before"].setdefault(session.id, snap)
        if deleted:
            effects["sessions_deleted"].append(snap)

    def _undo_effects(self, effects: dict[str, Any]) -> None:
        if not effects:
            return
        for batch in reversed(list(effects.get("batches_approved") or [])):
            self.calendar.undo_batch(batch)
        for batch in effects.get("batches_proposed") or []:
            self.calendar.reject_batch(batch)
        for sid in effects.get("sessions_created") or []:
            self.calendar.delete_session(sid)
        for snap in effects.get("sessions_deleted") or []:
            if isinstance(snap, dict):
                self.calendar.restore_session_snapshot(snap)
        for snap in (effects.get("sessions_before") or {}).values():
            if isinstance(snap, dict):
                self.calendar.restore_session_snapshot(snap)
        for snap in (effects.get("goals_before") or {}).values():
            if isinstance(snap, dict):
                self.goals.restore_snapshot(snap)
        for gid in effects.get("goals_created") or []:
            self.goals.remove(gid)

    def _record_decision_turn(
        self,
        cid: str | None,
        *,
        decision: str,
        batch_id: str,
        effects: dict[str, Any],
    ) -> None:
        if not cid:
            return
        rid = f"decide-{uuid.uuid4()}"
        self.turns.try_begin(rid, cid, f"{decision}:{batch_id}")
        self.turns.update(rid, status="completed", effects_json=effects)

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
        cid = conversation_id
        if not cid:
            sessions = self.calendar.list_proposed_for_batch(batch_id)
            if sessions and sessions[0].goal_id:
                goal = self.goals.get(sessions[0].goal_id)
                cid = goal.conversation_id if goal else None
        if decision == "approve":
            booked = self.calendar.approve_batch(batch_id, conversation_id=cid)
            effects = self._empty_effects()
            if booked:
                effects["batches_approved"].append(batch_id)
            self._record_decision_turn(cid, decision=decision, batch_id=batch_id, effects=effects)
            return {
                "booked": booked,
                "undo_batch_id": batch_id if booked else None,
                "proposal_groups": self._proposal_groups_for(cid) if cid else [],
            }
        if decision == "reject":
            effects = self._empty_effects()
            for session in self.calendar.list_proposed_for_batch(batch_id):
                self._snapshot_session(effects, session)
            effects["batches_rejected"].append(batch_id)
            n = self.calendar.reject_batch(batch_id)
            self._record_decision_turn(cid, decision=decision, batch_id=batch_id, effects=effects)
            return {
                "rejected": n,
                "proposal_groups": self._proposal_groups_for(cid) if cid else [],
            }
        if decision == "undo":
            restored = self.calendar.undo_batch(batch_id)
            return {
                "undone": restored,
                "proposal_groups": self._proposal_groups_for(cid) if cid else [],
            }
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
            if client is not None and entry.get("owns_http_client"):
                try:
                    client.close()
                except Exception:
                    pass
                entry["http_client"] = None
        turn = self.turns.get(request_id)
        status = turn["status"] if turn else None
        if status in {"committing", "completed"}:
            committed = True
        elif turn and status not in {"cancelled"}:
            self.turns.update(request_id, status="cancelled")
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
        """Rebuild unfinished chat state: proposal groups, previews, questions, answers."""
        pending = self.turns.latest_awaiting_approval(conversation_id)
        extra: dict[str, Any] = {
            "mutation_preview": None,
            "clarification_questions": [],
            "entered_answers": {},
            "proposal_groups": [],
        }
        if pending:
            extra["mutation_preview"] = self._preview_from_pending(pending)
            extra["clarification_questions"] = pending.get("clarifications") or []
        conv = self.conversations.get(conversation_id)
        draft = (conv or {}).get("draft") or {}
        if draft.get("clarification_questions") and not extra["clarification_questions"]:
            extra["clarification_questions"] = draft["clarification_questions"]
        answers = draft.get("clarification_answers") or draft.get("answers") or {}
        if isinstance(answers, dict):
            extra["entered_answers"] = answers
            extra["clarification_answers"] = answers
        groups = self._proposal_groups_for(conversation_id)
        extra["proposal_groups"] = [g.model_dump(mode="json") for g in groups]
        if not groups:
            latest_turn = self.turns.latest_for_conversation(conversation_id)
            stored = (latest_turn or {}).get("response") or {}
            if stored.get("clarification_questions") and not extra["clarification_questions"]:
                extra["clarification_questions"] = stored["clarification_questions"]
            return {
                "goal": None,
                "proposed_sessions": [],
                "proposal_summary": None,
                **extra,
            }
        first = groups[0]
        return {
            "goal": first.goal,
            "proposed_sessions": first.sessions,
            "proposal_summary": first.summary,
            **extra,
        }

    def _proposal_groups_for(self, conversation_id: str | None) -> list[ProposalGroup]:
        if not conversation_id:
            return []
        goals = sorted(
            [g for g in self.goals.list_open() if g.conversation_id == conversation_id],
            key=lambda g: g.created_at or "",
        )
        groups: list[ProposalGroup] = []
        for goal in goals:
            batch_id = self.calendar.open_proposal_batch(goal.id)
            if not batch_id:
                continue
            sessions = self.calendar.list_proposed_for_batch(batch_id)
            if not sessions:
                continue
            summary = self._public_proposal_summary(summarize_proposal(goal, sessions))
            groups.append(
                ProposalGroup(
                    goal=goal,
                    proposal_batch_id=batch_id,
                    summary=summary,
                    sessions=sessions,
                )
            )
        return groups

    def _preview_from_pending(self, pending: dict[str, Any]) -> dict[str, Any] | None:
        ops = pending.get("pending_ops") or []
        if not ops:
            return None
        first = ops[0]
        if isinstance(first, dict) and first.get("preview"):
            return first["preview"]
        records = []
        for raw in ops:
            records.extend((raw.get("preview") or {}).get("records") or [])
        if not records:
            return None
        return preview_payload(
            kind=first.get("kind") if isinstance(first, dict) else "calendar",
            records=records,
            why="Waiting for approval before changing the calendar.",
        )

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
        self._active_requests[rid] = {"cancelled": False, "http_client": None, "owns_http_client": False}
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

        answers = []
        for item in clarification_answers:
            if hasattr(item, "model_dump"):
                answers.append(item.model_dump())
            elif isinstance(item, dict):
                answers.append(item)

        revision_group = None
        if revision_of and not (existing and existing.get("user_message_id")):
            reverted = self.revert_to(cid, revision_of, include_target=True)
            if not reverted.get("ok"):
                raise RevertBlocked(reverted.get("detail") or "Can't edit that message.")
            revision_group = reverted.get("revision_group")

        begun = existing or self.turns.try_begin(request_id, cid, text)
        if begun is None:
            stored = self.turns.get(request_id)
            if stored and stored.get("response"):
                return ChatResponse.model_validate(stored["response"])
            begun = stored

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
            status = "awaiting_approval" if response.mutation_preview else "completed"
            return self._finish_response(request_id, cid, response, status=status)
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
        if pending:
            # A new instruction supersedes an unapproved preview without applying it.
            self.turns.update(pending["request_id"], status="cancelled", pending_ops_json=[])

        open_groups = self._proposal_groups_for(cid)
        if len(open_groups) == 1 and lower in approve_words:
            batch_id = open_groups[0].proposal_batch_id
            booked = self.calendar.approve_batch(batch_id, conversation_id=cid)
            title = open_groups[0].goal.title if open_groups[0].goal else "your goal"
            reply = (
                f"Booked {len(booked)} session(s) once each and linked them to {title}. "
                "Tell me when one is completed or missed and I'll record it."
            )
            self._add_message(cid, "assistant", reply)
            response = ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=open_groups[0].goal,
                booked_sessions=booked,
                ai_available=True,
                undo_batch_id=batch_id if booked else None,
                proposal_groups=[],
                request_id=request_id,
                activity=[
                    {"stage": "completed", "label": "Booked after your approval", "detail": "No Cloud AI used"},
                ],
            )
            effects = self._empty_effects()
            if booked:
                effects["batches_approved"].append(batch_id)
            return self._finish_response(request_id, cid, response, effects=effects)
        if len(open_groups) == 1 and lower in reject_words:
            batch_id = open_groups[0].proposal_batch_id
            effects = self._empty_effects()
            for session in self.calendar.list_proposed_for_batch(batch_id):
                self._snapshot_session(effects, session)
            effects["batches_rejected"].append(batch_id)
            n = self.calendar.reject_batch(batch_id)
            reply = f"Rejected {n} proposed session(s). Say what to change and I'll propose again."
            self._add_message(cid, "assistant", reply)
            response = ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=open_groups[0].goal,
                ai_available=True,
                proposal_groups=[],
                request_id=request_id,
            )
            return self._finish_response(request_id, cid, response, effects=effects)

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
            response = ChatResponse(
                conversation_id=cid,
                reply="Stopped. Cloud AI output was discarded — no calendar changes.",
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
        if self._is_cancelled(request_id):
            response = ChatResponse(
                conversation_id=cid,
                reply="This turn was already saving — some calendar changes may have been applied.",
                goal=goal,
                ai_available=True,
                request_id=request_id,
                stopped=True,
                stop_committed=True,
                activity=[
                    {
                        "stage": "completed",
                        "label": "Finished during Stop",
                        "detail": "Changes may have been applied",
                    }
                ],
            )
            return self._finish_response(request_id, cid, response, status="completed")
        response = self._apply_turn(
            cid,
            turn,
            goal,
            request_id=request_id,
            context_categories=context_categories,
            user_text=text,
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
        effects: dict[str, Any] | None = None,
    ) -> ChatResponse:
        response.request_id = request_id
        if not response.proposal_groups:
            response.proposal_groups = self._proposal_groups_for(cid)
        payload = response.model_dump(mode="json")
        fields: dict[str, Any] = {"status": status, "response_json": payload}
        if effects is not None:
            fields["effects_json"] = effects
        self.turns.update(request_id, **fields)
        if status in {"completed", "cancelled", "failed"} and not response.clarification_questions:
            conv = self.conversations.get(cid) or {}
            draft = dict(conv.get("draft") or {})
            draft["clarification_questions"] = []
            if status != "awaiting_approval":
                draft["clarification_answers"] = {}
                draft["answers"] = {}
            self.conversations.save_draft(cid, draft)
        elif response.clarification_questions:
            conv = self.conversations.get(cid) or {}
            draft = dict(conv.get("draft") or {})
            draft["clarification_questions"] = [q.model_dump() for q in response.clarification_questions]
            self.conversations.save_draft(cid, draft)
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
        stale = False
        refreshed_records: list[dict[str, Any]] = []
        refreshed_ops: list[dict[str, Any]] = []
        effects = self._empty_effects()

        for raw in ops:
            if raw.get("kind") == "pause_others":
                keep_id = raw.get("target_id") or (goal.id if goal else None)
                if keep_id:
                    for other in self.goals.list_open():
                        if other.conversation_id == cid and other.id != keep_id:
                            self._record_goal_before(effects, other)
                    self.goals.pause_others(cid, keep_id=keep_id)
                continue
            action_data = raw.get("calendar_action") or {}
            try:
                action = CalendarAction.model_validate(action_data)
            except ValidationError:
                unresolved.append("invalid_preview")
                continue
            reviewed_ids = list(raw.get("reviewed_ids") or [])
            fingerprints = raw.get("fingerprints") or {}
            if not reviewed_ids:
                unresolved.append("preview_missing_ids")
                stale = True
                continue
            current = []
            for sid in reviewed_ids:
                session = self.calendar.get_session(sid)
                if session is None or not self.calendar.fingerprints_match(session, fingerprints.get(sid)):
                    stale = True
                    break
                current.append(session)
            if stale:
                # Rebuild a preview from the original selector for the user to review again.
                targets = self.calendar.resolve_action_targets(action)
                records = [self._preview_record(t, action) for t in targets]
                preview = preview_payload(
                    kind=raw.get("kind") or f"calendar_{action.op}",
                    records=records,
                    why="The calendar changed after this preview. Approve again to apply only the sessions shown.",
                )
                refreshed_ops.append(self._pending_op(raw.get("kind") or f"calendar_{action.op}", action, records, preview))
                refreshed_records.extend(records)
                continue
            for session in current:
                self._snapshot_session(effects, session, deleted=action.op == "delete")
            d, u, errors = self.calendar.apply_reviewed_action(action, reviewed_ids)
            deleted.extend(d)
            updated.extend(u)
            unresolved.extend(errors)

        if stale:
            preview = preview_payload(
                kind="calendar",
                records=refreshed_records,
                why="The calendar changed after this preview. Nothing was applied.",
            )
            self.turns.update(pending["request_id"], status="cancelled", pending_ops_json=[])
            self.turns.update(request_id, pending_ops_json=refreshed_ops)
            reply = (
                "That preview is stale — matching calendar records changed, so I didn't apply it. "
                "Review the updated list and approve only if it still looks right."
            )
            self._add_message(cid, "assistant", reply)
            return ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                ai_available=True,
                request_id=request_id,
                mutation_preview=preview,
                unresolved=unresolved,
                activity=[
                    {"stage": "preview", "label": "Preview is stale", "detail": "No calendar changes applied"},
                    {"stage": "awaiting_approval", "label": "Review the updated preview", "detail": None},
                ],
            )

        self.turns.update(pending["request_id"], status="completed", pending_ops_json=[])
        if request_id:
            self.turns.update(request_id, effects_json=effects)
        n = len(deleted) + len(updated)
        reply = f"Applied {n} calendar change{'s' if n != 1 else ''}."
        if unresolved:
            reply += " Some reviewed items could not be applied."
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
        open_goals = [g.model_dump() for g in self.goals.list_open() if g.conversation_id == cid][:12]
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
        request_client = None
        if entry is not None and hasattr(provider, "open_request_client"):
            try:
                request_client = provider.open_request_client()
                entry["http_client"] = request_client
                entry["owns_http_client"] = True
            except Exception:
                request_client = None

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
        complete_kwargs: dict[str, Any] = {"allow_retry": False, "cancel_check": _cancelled}
        if request_client is not None:
            complete_kwargs["http_client"] = request_client
        try:
            try:
                raw = provider.complete_json(SYSTEM_PROMPT, payload, **complete_kwargs)
            except TypeError:
                raw = provider.complete_json(SYSTEM_PROMPT, payload, allow_retry=False)
            try:
                parsed = parse_buddy_turn(raw)
            except ValidationError as parse_exc:
                paths = [".".join(str(part) for part in err.get("loc", ())) for err in parse_exc.errors()[:12]]
                log.warning("buddy_turn_invalid paths=%s", paths)
                raise GroqError("malformed", "Cloud AI returned an invalid BuddyTurn") from parse_exc
            except ValueError as parse_exc:
                log.warning("buddy_turn_invalid detail=%s", str(parse_exc)[:120])
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
                    raw2 = provider.complete_json(SYSTEM_PROMPT, repair, **complete_kwargs)
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
        finally:
            if request_client is not None:
                if entry is not None and entry.get("http_client") is request_client:
                    entry["http_client"] = None
                    entry["owns_http_client"] = False
                try:
                    request_client.close()
                except Exception:
                    pass

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
        user_text: str = "",
    ) -> ChatResponse:
        proposed: list[SessionOut] = []
        booked: list[SessionOut] = []
        captured: list[Spark] = []
        unresolved: list[str] = []
        deleted_session_ids: list[str] = []
        updated_sessions: list[SessionOut] = []
        operations: list[dict[str, Any]] = []
        touched_goals: list[Goal] = []
        created_refs: dict[str, Goal] = {}
        proposal_groups: list[ProposalGroup] = []
        undo_batch_id: str | None = None
        pending_ops: list[dict[str, Any]] = []
        preview_records: list[dict[str, Any]] = []
        questions = list(turn.clarification_questions)
        open_goals = [g for g in self.goals.list_open() if g.conversation_id == cid]
        effects = self._empty_effects()

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

        raw_ops = operations_from_turn(turn)
        for op in raw_ops:
            if self._is_cancelled(request_id):
                operations.append(
                    {
                        "id": op.get("id") or str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "cancelled",
                        "goal_id": goal.id if goal else None,
                        "detail": "Stopped before this operation",
                    }
                )
                break
            cal_action = operation_calendar_action(op)
            targets = self.calendar.resolve_action_targets(cal_action) if cal_action else []
            policy = classify_operation(
                kind=op["kind"],
                target_count=len(targets),
                all_matching=bool(cal_action.all_matching) if cal_action else False,
                has_session_id=bool(cal_action and cal_action.session_id) or bool(
                    (op.get("payload") or {}).get("session_id") or op.get("target_id")
                ),
                confidence=float(op.get("confidence") or turn.confidence),
            )
            if policy == "clarify":
                operations.append(
                    {
                        "id": op.get("id") or str(uuid.uuid4()),
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
                records = [self._preview_record(t, cal_action) for t in targets] if cal_action else []
                payload = preview_payload(
                    kind=op["kind"],
                    records=records,
                    why="Destructive or bulk calendar changes wait for your approval.",
                )
                pending_ops.append(self._pending_op(op["kind"], cal_action, records, payload, op))
                preview_records.extend(records)
                operations.append(
                    {
                        "id": op.get("id") or str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "needs_approval",
                        "goal_id": goal.id if goal else None,
                        "detail": payload["why"],
                        "preview": payload,
                    }
                )
                continue

            try:
                outcome = self._apply_operation(
                    cid,
                    op,
                    goal,
                    touched_goals,
                    created_refs=created_refs,
                    user_text=user_text,
                    open_goals=open_goals,
                    effects=effects,
                )
            except Exception as exc:  # noqa: BLE001
                operations.append(
                    {
                        "id": op.get("id") or str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "failed",
                        "goal_id": goal.id if goal else None,
                        "detail": exc.__class__.__name__,
                    }
                )
                unresolved.append(op["kind"])
                log.exception("turn_op_failed kind=%s", op["kind"])
                continue

            if outcome.get("status") == "needs_clarification":
                operations.append(
                    {
                        "id": op.get("id") or str(uuid.uuid4()),
                        "kind": op["kind"],
                        "status": "needs_clarification",
                        "goal_id": outcome.get("goal_id") or (goal.id if goal else None),
                        "detail": outcome.get("detail"),
                    }
                )
                if outcome.get("question"):
                    questions.append(outcome["question"])
                elif not questions:
                    questions.append(
                        ClarificationQuestion(
                            id="which-goal",
                            label=outcome.get("detail") or "Which goal should I update?",
                            answer_type="short_text",
                        )
                    )
                continue

            if outcome.get("goal") is not None:
                goal = outcome["goal"]
                if goal and all(g.id != goal.id for g in touched_goals):
                    touched_goals.append(goal)
                open_goals = [g for g in self.goals.list_open() if g.conversation_id == cid]
            if outcome.get("ref") and outcome.get("goal"):
                created_refs[str(outcome["ref"])] = outcome["goal"]
            proposed.extend(outcome.get("proposed") or [])
            booked.extend(outcome.get("booked") or [])
            captured.extend(outcome.get("sparks") or [])
            deleted_session_ids.extend(outcome.get("deleted") or [])
            updated_sessions.extend(outcome.get("updated") or [])
            unresolved.extend(outcome.get("errors") or [])
            if outcome.get("proposal_group"):
                proposal_groups.append(outcome["proposal_group"])
            if outcome.get("undo_batch_id"):
                undo_batch_id = outcome["undo_batch_id"]
            operations.append(
                {
                    "id": op.get("id") or str(uuid.uuid4()),
                    "kind": op["kind"],
                    "status": outcome.get("status") or "succeeded",
                    "goal_id": (outcome.get("goal") or goal).id if (outcome.get("goal") or goal) else None,
                    "detail": outcome.get("detail"),
                }
            )

        if not proposal_groups:
            proposal_groups = self._dedupe_proposal_groups(proposed, touched_goals, goal)

        reply = self._honest_reply(turn.assistant_text or "", operations)
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
            reply = (
                f"Proposed plans for {len(proposal_groups) or 1} goal"
                f"{'s' if len(proposal_groups) != 1 else ''}. Nothing is booked until you approve."
            )

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
                kind=pending_ops[0].get("kind") or "calendar",
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
                    "detail": f"Prepared {len(proposed)} sessions across {len(proposal_groups) or 1} goal(s)",
                }
            )
            activity.append({"stage": "completed", "label": "Waiting for approval", "detail": "No calendar changes made"})
        elif questions:
            activity.append({"stage": "clarifying", "label": "Preparing clarifying questions", "detail": None})
            activity.append({"stage": "completed", "label": "Completed", "detail": "No calendar changes made"})
        else:
            activity.append({"stage": "completed", "label": "Completed", "detail": "No calendar changes made"})

        if request_id and pending_ops:
            self.turns.update(
                request_id,
                pending_ops_json=pending_ops,
                clarifications_json=[q.model_dump() for q in questions],
            )
        elif request_id and questions:
            self.turns.update(request_id, clarifications_json=[q.model_dump() for q in questions])

        if request_id:
            self.turns.update(request_id, effects_json=effects)

        self._add_message(cid, "assistant", reply)
        pending_q = None
        if pending_ops:
            pending_q = "Approve or cancel these calendar changes?"
        elif proposed:
            pending_q = "Approve or reject these proposed sessions?"
        else:
            pending_q = turn.clarification
        display_goal = goal
        if proposal_groups and len({g.goal.id for g in proposal_groups if g.goal}) > 1:
            display_goal = None
        return ChatResponse(
            conversation_id=cid,
            reply=reply,
            goal=display_goal,
            pending_question=pending_q,
            proposed_sessions=proposed,
            booked_sessions=booked,
            sparks=captured,
            unresolved=unresolved,
            ai_available=True,
            proposal_summary=proposal_groups[0].summary if proposal_groups else None,
            proposal_groups=proposal_groups,
            clarification_questions=questions,
            activity=activity,
            undo_batch_id=undo_batch_id,
            deleted_session_ids=deleted_session_ids,
            updated_sessions=updated_sessions,
            operations=operations,
            mutation_preview=mutation_preview,
        )

    def _dedupe_proposal_groups(
        self,
        proposed: list[SessionOut],
        touched_goals: list[Goal],
        goal: Goal | None,
    ) -> list[ProposalGroup]:
        by_batch: dict[str, list[SessionOut]] = {}
        for session in proposed:
            if not session.proposal_batch_id:
                continue
            by_batch.setdefault(session.proposal_batch_id, []).append(session)
        groups: list[ProposalGroup] = []
        goals_by_id = {g.id: g for g in touched_goals}
        if goal:
            goals_by_id[goal.id] = goal
        for batch_id, sessions in by_batch.items():
            gid = next((s.goal_id for s in sessions if s.goal_id), None)
            target = goals_by_id.get(gid) if gid else None
            if target is None and gid:
                target = self.goals.get(gid)
            summary = self._public_proposal_summary(summarize_proposal(target, sessions) if target else None)
            groups.append(
                ProposalGroup(
                    goal=target,
                    proposal_batch_id=batch_id,
                    summary=summary,
                    sessions=sessions,
                )
            )
        return groups

    def _honest_reply(self, turn_text: str, operations: list[dict[str, Any]]) -> str:
        text = (turn_text or "").strip()
        statuses = [o.get("status") for o in operations]
        failed = [o for o in operations if o.get("status") in {"failed", "needs_clarification"}]
        succeeded = [o for o in operations if o.get("status") == "succeeded"]
        approval = [o for o in operations if o.get("status") == "needs_approval"]
        success_claim = bool(re.search(
            r"\b(all done|everything succeeded|everything worked|all succeeded)\b",
            text,
            re.I,
        ))
        if not operations:
            return text
        if failed and not succeeded and not approval:
            return "I couldn't apply that — " + "; ".join(
                f"{o.get('kind')}: {o.get('detail') or 'not valid'}" for o in failed
            )
        if (failed and succeeded) or (failed and success_claim) or (approval and success_claim):
            parts = []
            for op in operations:
                kind = str(op.get("kind") or "action").replace("_", " ")
                status = op.get("status")
                detail = op.get("detail")
                if status == "succeeded":
                    parts.append(f"{kind} succeeded" + (f" ({detail})" if detail else ""))
                elif status == "needs_approval":
                    parts.append(f"{kind} is waiting for your approval")
                elif status == "needs_clarification":
                    parts.append(f"{kind} needs a bit more detail" + (f": {detail}" if detail else ""))
                elif status == "failed":
                    parts.append(f"{kind} failed" + (f": {detail}" if detail else ""))
            prefix = "I couldn't apply everything. "
            return prefix + "; ".join(parts) + "."
        return text

    def _preview_record(self, session: SessionOut, action: CalendarAction | None) -> dict[str, Any]:
        before = {
            "title": session.title,
            "start_at": session.start_at,
            "end_at": session.end_at,
            "status": session.status,
            "updated_at": session.updated_at,
        }
        after = None
        if action:
            if action.op == "delete":
                after = None
            elif action.op == "update":
                after = {
                    "title": action.new_title or session.title,
                    "start_at": action.new_start_at or session.start_at,
                    "end_at": action.new_end_at or session.end_at,
                    "status": session.status,
                }
            elif action.op == "move":
                after = {
                    "title": session.title,
                    "start_at": action.new_start_at or session.start_at,
                    "end_at": action.new_end_at or session.end_at,
                    "status": session.status,
                }
            elif action.op == "mark_outcome":
                after = {**before, "status": action.outcome}
        reasons: list[str] = []
        if action and action.session_id:
            reasons.append("Exact session from this preview")
        if action and action.title_contains:
            reasons.append(f"Title contains “{action.title_contains}”")
        if action and action.date:
            reasons.append(f"On {action.date}")
        if action and action.goal_id:
            reasons.append("Linked to the selected goal")
        if action and action.all_matching:
            reasons.append("All matching sessions")
        return {
            "id": session.id,
            "title": session.title,
            "start_at": session.start_at,
            "end_at": session.end_at,
            "status": session.status,
            "before": before,
            "after": after,
            "match_reason": "; ".join(reasons) or "Matched from your request",
            "fingerprint": before,
        }

    def _pending_op(
        self,
        kind: str,
        action: CalendarAction | None,
        records: list[dict[str, Any]],
        preview: dict[str, Any],
        op: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        fingerprints = {
            rec["id"]: rec.get("fingerprint") or rec.get("before") or {}
            for rec in records
            if rec.get("id")
        }
        return {
            "kind": kind,
            "target_id": (op or {}).get("target_id"),
            "calendar_action": action.model_dump() if action else None,
            "reviewed_ids": [rec["id"] for rec in records if rec.get("id")],
            "fingerprints": fingerprints,
            "preview": preview,
        }

    def _resolve_goal_for_op(
        self,
        cid: str,
        op: dict[str, Any],
        *,
        current: Goal | None,
        created_refs: dict[str, Goal],
        open_goals: list[Goal],
        user_text: str,
        require: bool,
    ) -> tuple[Goal | None, dict[str, Any] | None]:
        payload = op.get("payload") or {}
        target_id = op.get("target_id") or payload.get("goal_id")
        if target_id:
            found = self.goals.get(str(target_id))
            if found and found.conversation_id == cid:
                return found, None
        ref = op.get("target_ref")
        if ref and ref in created_refs:
            return created_refs[ref], None
        hay_parts = [
            user_text,
            payload.get("title"),
            payload.get("domain"),
            ref,
        ]
        hay = " ".join(str(p) for p in hay_parts if p).lower()
        scored: list[tuple[int, Goal]] = []
        for goal in open_goals:
            score = self._goal_match_score(goal, hay)
            if score > 0:
                scored.append((score, goal))
        scored.sort(key=lambda item: item[0], reverse=True)
        if len(scored) == 1 or (len(scored) > 1 and scored[0][0] >= scored[1][0] + 3):
            return scored[0][1], None
        if len(scored) > 1:
            return None, {
                "status": "needs_clarification",
                "detail": "Several goals could match that follow-up — which one?",
                "question": ClarificationQuestion(
                    id="which-goal",
                    label="Which goal should I update?",
                    answer_type="single_choice",
                    options=[g.title for _, g in scored[:4]],
                ),
            }
        conv_goals = [g for g in open_goals if g.conversation_id == cid] or list(open_goals)
        if len(conv_goals) == 1:
            return conv_goals[0], None
        if current and current.conversation_id == cid and self._looks_like_slot_fill(hay):
            return current, None
        if not require:
            if current and current.conversation_id == cid:
                return current, None
            return None, None
        return None, {
            "status": "needs_clarification",
            "detail": "I couldn't tell which goal that refers to.",
            "question": ClarificationQuestion(
                id="which-goal",
                label="Which goal should I update?",
                answer_type="short_text",
            ),
        }

    @staticmethod
    def _looks_like_slot_fill(hay: str) -> bool:
        if not hay:
            return False
        hints = (
            "week",
            "twice",
            "times",
            "hour",
            "deadline",
            "minute",
            "v1",
            "v2",
            "v3",
            "v4",
            "v5",
            "v6",
            "v7",
            "baseline",
            "frequency",
        )
        return any(h in hay for h in hints)

    @staticmethod
    def _goal_match_score(goal: Goal, hay: str) -> int:
        if not hay:
            return 0
        score = 0
        title = (goal.title or "").lower()
        domain = (goal.domain or "").lower()
        if title and title in hay:
            score += 6
        if domain and domain in hay:
            score += 6
        tokens = set(re.findall(r"[a-z0-9]+", f"{title} {domain}"))
        hay_tokens = set(re.findall(r"[a-z0-9]+", hay))
        for token in tokens:
            if len(token) < 4:
                continue
            if token in hay_tokens or token in hay:
                score += 2
            for word in hay_tokens:
                if token.startswith(word[:4]) or word.startswith(token[:4]):
                    score += 2
                    break
        if "climb" in title or "climb" in domain:
            if "climb" in hay:
                score += 4
        if "read" in title or "read" in domain or "book" in title:
            if "read" in hay or "book" in hay:
                score += 4
        if "save" in title or "saving" in domain or "£" in (goal.title or ""):
            if "save" in hay or "saving" in hay:
                score += 4
        return score

    def _apply_operation(
        self,
        cid: str,
        op: dict[str, Any],
        goal: Goal | None,
        touched_goals: list[Goal],
        *,
        created_refs: dict[str, Goal],
        user_text: str,
        open_goals: list[Goal],
        effects: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        kind = op["kind"]
        payload = op.get("payload") or {}
        update = operation_goal_update(op) if kind in {"goal_create", "goal_update", "pause_others"} else None
        cal_action = operation_calendar_action(op)
        effects = effects if effects is not None else self._empty_effects()

        if kind == "goal_create" and update is not None:
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
            effects["goals_created"].append(created.id)
            return {
                "goal": created,
                "status": "succeeded",
                "detail": created.title,
                "ref": op.get("target_ref"),
            }

        if kind in {"goal_update", "pause_others"} and update is not None:
            target, clarify = self._resolve_goal_for_op(
                cid,
                op,
                current=goal,
                created_refs=created_refs,
                open_goals=open_goals,
                user_text=user_text,
                require=True,
            )
            if clarify:
                return clarify
            if target is None:
                return {"status": "needs_clarification", "detail": "Which goal should I update?"}
            if kind == "pause_others":
                for other in self.goals.list_open():
                    if other.conversation_id == cid and other.id != target.id:
                        self._record_goal_before(effects, other)
                self.goals.pause_others(cid, keep_id=target.id)
            self._record_goal_before(effects, target)
            updated = self.goals.apply_update(target, update)
            return {"goal": updated, "status": "succeeded", "detail": updated.title}

        if kind == "spark_capture":
            content = payload.get("spark_content") or op.get("spark_content")
            if not content:
                return {"status": "failed", "detail": "missing spark content"}
            spark = self.sparks.capture(content)
            return {"sparks": [spark], "status": "succeeded", "detail": spark.content}

        if kind == "spark_dismiss":
            spark_id = payload.get("spark_id") or op.get("target_id")
            if spark_id:
                self.sparks.dismiss(spark_id)
            return {"status": "succeeded"}

        if kind == "spark_promote":
            spark_id = payload.get("spark_id") or op.get("target_id")
            target, _ = self._resolve_goal_for_op(
                cid, op, current=goal, created_refs=created_refs, open_goals=open_goals, user_text=user_text, require=False
            )
            if spark_id and target:
                self.sparks.promote(spark_id, target.id)
            return {"status": "succeeded", "goal": target or goal}

        if kind == "propose_sessions":
            target, clarify = self._resolve_goal_for_op(
                cid,
                op,
                current=goal,
                created_refs=created_refs,
                open_goals=[*touched_goals, *open_goals],
                user_text=user_text,
                require=False,
            )
            if clarify:
                return clarify
            if target is None and len(touched_goals) == 1:
                target = touched_goals[0]
            if target is None and goal and len(open_goals) <= 1:
                target = goal
            if target is None:
                return {
                    "status": "needs_clarification",
                    "detail": "Which goal should I propose sessions for?",
                    "question": ClarificationQuestion(
                        id="which-goal",
                        label="Which goal should I put on the calendar?",
                        answer_type="short_text",
                    ),
                }
            if target.status == "gathering" and not self._goal_has_cadence(target):
                return {"goal": target, "status": "skipped", "detail": "no cadence"}
            self._record_goal_before(effects, target)
            _reply_text, sessions, raw_summary = propose_for_goal(self.calendar, target)
            self.goals.save(target)
            summary = self._public_proposal_summary(raw_summary)
            group = None
            if sessions:
                batch_id = sessions[0].proposal_batch_id or ""
                group = ProposalGroup(
                    goal=target,
                    proposal_batch_id=batch_id,
                    summary=summary,
                    sessions=sessions,
                )
                for session in sessions:
                    effects["sessions_created"].append(session.id)
                if batch_id and batch_id not in effects["batches_proposed"]:
                    effects["batches_proposed"].append(batch_id)
            status = "succeeded" if sessions else "skipped"
            return {
                "goal": target,
                "proposed": sessions,
                "proposal_group": group,
                "proposal_summary": summary,
                "status": status,
                "detail": f"{len(sessions)} sessions" if sessions else "no cadence",
            }

        if kind == "approve_proposals":
            batch = payload.get("batch_id") or op.get("target_id")
            target, _ = self._resolve_goal_for_op(
                cid, op, current=goal, created_refs=created_refs, open_goals=open_goals, user_text=user_text, require=False
            )
            if not batch and target:
                batch = self.calendar.open_proposal_batch(target.id)
            booked: list[SessionOut] = []
            undo = None
            if batch:
                booked = self.calendar.approve_batch(batch, conversation_id=cid)
                undo = batch if booked else None
                if booked:
                    effects["batches_approved"].append(batch)
            return {"goal": target or goal, "booked": booked, "undo_batch_id": undo, "status": "succeeded"}

        if kind == "reject_proposals":
            batch = payload.get("batch_id") or op.get("target_id")
            target, _ = self._resolve_goal_for_op(
                cid, op, current=goal, created_refs=created_refs, open_goals=open_goals, user_text=user_text, require=False
            )
            if not batch and target:
                batch = self.calendar.open_proposal_batch(target.id)
            if batch:
                for session in self.calendar.list_proposed_for_batch(batch):
                    self._snapshot_session(effects, session)
                effects["batches_rejected"].append(batch)
                self.calendar.reject_batch(batch)
            return {"goal": target or goal, "status": "succeeded"}

        if kind == "session_outcome":
            session_id = payload.get("session_id") or op.get("target_id")
            outcome = payload.get("outcome")
            if session_id and outcome:
                self._snapshot_session(effects, self.calendar.get_session(session_id))
                marked = self.calendar.mark_outcome(session_id, outcome)
                return {"updated": [marked] if marked else [], "status": "succeeded", "goal": goal}
            if cal_action:
                for session in self.calendar.resolve_action_targets(cal_action):
                    self._snapshot_session(effects, session)
                deleted, updated, errors = self.calendar.apply_calendar_action(cal_action)
                return {
                    "deleted": deleted,
                    "updated": updated,
                    "errors": errors,
                    "status": "succeeded" if not errors else "failed",
                    "goal": goal,
                }

        if kind.startswith("calendar_") and cal_action:
            for session in self.calendar.resolve_action_targets(cal_action):
                self._snapshot_session(effects, session, deleted=cal_action.op == "delete")
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
