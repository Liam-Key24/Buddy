"""Goal persistence — SQLite only, no natural-language parsing."""

from __future__ import annotations

import json
import uuid
from datetime import datetime, timezone
from typing import Any

from .schemas import Goal, GoalStatus, GoalUpdate


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class GoalStore:
    def __init__(self, conn):
        self.conn = conn

    def get(self, goal_id: str) -> Goal | None:
        row = self.conn.execute("SELECT * FROM goals WHERE id = ?", (goal_id,)).fetchone()
        return self._row(row) if row else None

    def active_for_conversation(self, conversation_id: str) -> Goal | None:
        row = self.conn.execute(
            """
            SELECT * FROM goals
            WHERE conversation_id = ?
              AND status IN ('gathering', 'ready_to_plan', 'planned', 'active')
            ORDER BY updated_at DESC
            LIMIT 1
            """,
            (conversation_id,),
        ).fetchone()
        return self._row(row) if row else None

    def list_open(self) -> list[Goal]:
        rows = self.conn.execute(
            """
            SELECT * FROM goals
            WHERE status IN ('gathering', 'ready_to_plan', 'planned', 'active')
            ORDER BY updated_at DESC
            """
        ).fetchall()
        return [self._row(r) for r in rows]

    def list_managed(self) -> list[Goal]:
        """Open + paused + completed goals for the Goals page.

        Removed goals are hard-deleted (`remove`), so they never appear here.
        Goals from soft-deleted chats are hidden unless the goal is completed (`done`).
        """
        rows = self.conn.execute(
            """
            SELECT g.* FROM goals g
            LEFT JOIN conversations c ON c.id = g.conversation_id
            WHERE g.status IN (
              'gathering', 'ready_to_plan', 'planned', 'active', 'paused', 'done'
            )
              AND (g.status = 'done' OR c.deleted_at IS NULL)
            ORDER BY
              CASE g.status
                WHEN 'active' THEN 0
                WHEN 'planned' THEN 1
                WHEN 'ready_to_plan' THEN 2
                WHEN 'gathering' THEN 3
                WHEN 'paused' THEN 4
                WHEN 'done' THEN 5
                ELSE 6
              END,
              g.updated_at DESC
            """
        ).fetchall()
        return [self._row(r) for r in rows]

    def remove(self, goal_id: str) -> Goal | None:
        """Hard-delete a goal and every record that references it."""
        goal = self.get(goal_id)
        if not goal:
            return None
        now = _now()
        # Sessions: remove all statuses so the calendar is clean.
        self.conn.execute("DELETE FROM sessions WHERE goal_id=?", (goal_id,))
        # Approval undo history for this goal.
        self.conn.execute("DELETE FROM approval_events WHERE goal_id=?", (goal_id,))
        # Sparks that pointed at this goal — clear link and reopen if still promoted.
        self.conn.execute(
            """
            UPDATE sparks
            SET promoted_goal_id=NULL,
                status=CASE WHEN status='promoted' THEN 'open' ELSE status END,
                updated_at=?
            WHERE promoted_goal_id=?
            """,
            (now, goal_id),
        )
        self.conn.execute("DELETE FROM goals WHERE id=?", (goal_id,))
        self.conn.commit()
        return goal

    def pause_others(self, conversation_id: str, keep_id: str | None = None) -> None:
        now = _now()
        if keep_id:
            self.conn.execute(
                """
                UPDATE goals SET status='paused', updated_at=?
                WHERE conversation_id=? AND id!=? AND status IN
                  ('gathering','ready_to_plan','planned','active')
                """,
                (now, conversation_id, keep_id),
            )
        else:
            self.conn.execute(
                """
                UPDATE goals SET status='paused', updated_at=?
                WHERE conversation_id=? AND status IN
                  ('gathering','ready_to_plan','planned','active')
                """,
                (now, conversation_id),
            )
        self.conn.commit()

    def pause_for_conversation(self, conversation_id: str) -> None:
        """Pause every open goal tied to a conversation (e.g. chat soft-deleted)."""
        self.pause_others(conversation_id, keep_id=None)

    def create(
        self,
        conversation_id: str,
        *,
        title: str,
        domain: str | None = None,
        target: str | None = None,
        deadline: str | None = None,
        baseline: str | None = None,
        frequency: str | None = None,
        commitment: str | None = None,
        status: GoalStatus = "gathering",
        facts: dict[str, Any] | None = None,
        pause_others: bool = True,
    ) -> Goal:
        if pause_others:
            self.pause_others(conversation_id)
        gid = _new_id()
        now = _now()
        facts = facts or {}
        self.conn.execute(
            """
            INSERT INTO goals (
                id, conversation_id, title, domain, target, deadline,
                baseline, frequency, commitment, status, facts_json, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                gid,
                conversation_id,
                title,
                domain,
                target,
                deadline,
                baseline,
                frequency,
                commitment,
                status,
                json.dumps(facts),
                now,
                now,
            ),
        )
        self.conn.commit()
        goal = self.get(gid)
        assert goal is not None
        return goal

    def apply_update(self, goal: Goal, update: GoalUpdate) -> Goal:
        if update.action == "pause_others":
            self.pause_others(goal.conversation_id, keep_id=goal.id)
        fields = {
            "title": update.title,
            "domain": update.domain,
            "target": update.target,
            "deadline": update.deadline,
            "baseline": update.baseline,
            "frequency": update.frequency,
            "commitment": update.commitment,
            "status": update.status,
        }
        for key, value in fields.items():
            if value is not None:
                setattr(goal, key, value)
        if update.facts:
            merged = dict(goal.facts or {})
            merged.update(update.facts)
            goal.facts = merged
        # Promote readiness when cadence or a concrete weekly plan exists
        if goal.status == "gathering":
            facts = goal.facts or {}
            plan = facts.get("weekly_plan") if isinstance(facts, dict) else None
            has_plan = isinstance(plan, dict) and bool(plan.get("slots"))
            if goal.frequency or goal.commitment or has_plan:
                goal.status = "ready_to_plan"
        self.save(goal)
        return goal

    def save(self, goal: Goal) -> None:
        self.conn.execute(
            """
            UPDATE goals SET
                title=?, domain=?, target=?, deadline=?, baseline=?,
                frequency=?, commitment=?, status=?, facts_json=?, updated_at=?
            WHERE id=?
            """,
            (
                goal.title,
                goal.domain,
                goal.target,
                goal.deadline,
                goal.baseline,
                goal.frequency,
                goal.commitment,
                goal.status,
                json.dumps(goal.facts or {}),
                _now(),
                goal.id,
            ),
        )
        self.conn.commit()

    def _row(self, row: Any) -> Goal:
        d = dict(row)
        facts_raw = d.get("facts_json") or "{}"
        try:
            facts = json.loads(facts_raw)
        except json.JSONDecodeError:
            facts = {}
        return Goal(
            id=d["id"],
            conversation_id=d["conversation_id"],
            title=d["title"],
            domain=d.get("domain"),
            target=d.get("target"),
            deadline=d.get("deadline"),
            baseline=d.get("baseline"),
            frequency=d.get("frequency"),
            commitment=d.get("commitment"),
            status=d.get("status") or "gathering",
            facts=facts if isinstance(facts, dict) else {},
            created_at=d.get("created_at"),
            updated_at=d.get("updated_at"),
        )


def progress_summary(goal: Goal, counts: dict[str, int]) -> str:
    """Human progress line — never expose raw DB status names as the headline."""
    completed = counts.get("completed", 0)
    missed = counts.get("missed", 0)
    scheduled = counts.get("scheduled", 0)
    if goal.domain == "savings":
        saved = (goal.facts or {}).get("saved_so_far")
        target = None
        if goal.target:
            digits = "".join(ch for ch in goal.target if ch.isdigit())
            target = int(digits) if digits else None
        if saved is not None and target:
            remaining = max(0, target - int(saved))
            return f"£{saved} saved, £{remaining} remaining"
        if saved is not None:
            return f"£{saved} saved so far"
    if completed or missed or scheduled:
        bits = [f"{completed} completed"]
        if missed:
            bits.append(f"{missed} missed")
        if scheduled:
            bits.append(f"{scheduled} scheduled")
        return ", ".join(bits)
    if goal.baseline and goal.frequency:
        return f"from {goal.baseline}, {goal.frequency}"
    if goal.baseline:
        return f"starting from {goal.baseline}"
    return "getting clear on the plan"
