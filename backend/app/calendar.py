"""Buddy calendar: availability, dated proposals, approval, and booking."""

from __future__ import annotations

import re
import uuid
from datetime import date, datetime, timedelta, timezone
from typing import Any

from .schemas import SessionOut


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


def frequency_to_weekly_count(frequency: str | None, commitment: str | None = None) -> int:
    text = f"{commitment or ''} {frequency or ''}".lower()
    if "every day" in text or "daily" in text:
        return 7
    m = re.search(r"(\d+)\s+times?\s+(?:a|per)\s+week", text)
    if m:
        return max(1, int(m.group(1)))
    if "twice" in text or "two times" in text:
        return 2
    if "once" in text or "one time" in text:
        return 1
    if "three" in text:
        return 3
    return 2


def session_duration_minutes(domain: str | None) -> int:
    if domain == "climbing":
        return 90
    if domain == "reading":
        return 45
    if domain == "savings":
        return 30
    return 60


def _parse_deadline_end(deadline: str | None) -> date:
    today = datetime.now().date()
    if not deadline:
        return today + timedelta(weeks=8)
    try:
        year_s, month_s = deadline.split("-")[:2]
        year, month = int(year_s), int(month_s)
        # last day of month
        if month == 12:
            return date(year, 12, 31)
        return date(year, month + 1, 1) - timedelta(days=1)
    except Exception:
        return today + timedelta(weeks=8)


def _overlaps(a_start: datetime, a_end: datetime, b_start: datetime, b_end: datetime) -> bool:
    return a_start < b_end and b_start < a_end


class CalendarService:
    def __init__(self, conn):
        self.conn = conn

    def list_sessions(
        self,
        *,
        start: str | None = None,
        end: str | None = None,
        statuses: list[str] | None = None,
    ) -> list[SessionOut]:
        sql = "SELECT * FROM sessions WHERE 1=1"
        params: list[Any] = []
        if start:
            sql += " AND end_at >= ?"
            params.append(start)
        if end:
            sql += " AND start_at <= ?"
            params.append(end)
        if statuses:
            sql += f" AND status IN ({','.join('?' for _ in statuses)})"
            params.extend(statuses)
        sql += " ORDER BY start_at ASC"
        rows = self.conn.execute(sql, params).fetchall()
        return [self._row_to_session(r) for r in rows]

    def list_fixed_blocks(self) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            "SELECT * FROM fixed_blocks ORDER BY weekday, start_minute"
        ).fetchall()
        return [dict(r) for r in rows]

    def _busy_intervals(self, day: date) -> list[tuple[datetime, datetime]]:
        busy: list[tuple[datetime, datetime]] = []
        weekday = day.weekday()
        for block in self.conn.execute(
            "SELECT * FROM fixed_blocks WHERE weekday = ?", (weekday,)
        ).fetchall():
            start = datetime(day.year, day.month, day.day, 0, 0) + timedelta(
                minutes=block["start_minute"]
            )
            end = datetime(day.year, day.month, day.day, 0, 0) + timedelta(
                minutes=block["end_minute"]
            )
            busy.append((start, end))

        day_start = datetime(day.year, day.month, day.day, 0, 0)
        day_end = day_start + timedelta(days=1)
        rows = self.conn.execute(
            """
            SELECT start_at, end_at FROM sessions
            WHERE status IN ('proposed', 'scheduled', 'completed')
              AND start_at < ? AND end_at > ?
            """,
            (day_end.isoformat(), day_start.isoformat()),
        ).fetchall()
        for r in rows:
            busy.append(
                (
                    datetime.fromisoformat(r["start_at"]),
                    datetime.fromisoformat(r["end_at"]),
                )
            )
        busy.sort(key=lambda x: x[0])
        return busy

    def find_free_slots(
        self,
        *,
        start_day: date,
        end_day: date,
        duration_minutes: int,
        prefer_hours: tuple[int, int] = (17, 21),
        limit: int = 40,
    ) -> list[tuple[datetime, datetime]]:
        slots: list[tuple[datetime, datetime]] = []
        day = start_day
        while day <= end_day and len(slots) < limit:
            busy = self._busy_intervals(day)
            window_start = datetime(day.year, day.month, day.day, prefer_hours[0], 0)
            window_end = datetime(day.year, day.month, day.day, prefer_hours[1], 0)
            cursor = window_start
            while cursor + timedelta(minutes=duration_minutes) <= window_end:
                candidate_end = cursor + timedelta(minutes=duration_minutes)
                conflict = any(_overlaps(cursor, candidate_end, b0, b1) for b0, b1 in busy)
                if not conflict:
                    slots.append((cursor, candidate_end))
                    break
                cursor += timedelta(minutes=30)
            day += timedelta(days=1)
        return slots

    def propose_goal_sessions(self, goal: Any, *, weeks: int | None = None) -> list[SessionOut]:
        """Create dated proposed sessions from real availability. Replaces open proposals for the goal."""
        # Drop previous open proposals for this goal (re-propose cleanly).
        self.conn.execute(
            """
            DELETE FROM sessions
            WHERE goal_id = ? AND status = 'proposed'
            """,
            (goal.id,),
        )

        weekly = frequency_to_weekly_count(goal.frequency, goal.commitment)
        duration = session_duration_minutes(goal.domain)
        today = datetime.now().date()
        # start from tomorrow so proposals are actionable upcoming sessions
        start_day = today + timedelta(days=1)
        end_day = _parse_deadline_end(goal.deadline)
        if weeks:
            end_day = min(end_day, today + timedelta(weeks=weeks))
        # Cap proposal horizon to 4 weeks for readable approval batches
        end_day = min(end_day, today + timedelta(weeks=4))

        needed = weekly * max(1, ((end_day - start_day).days // 7) + 1)
        needed = min(needed, weekly * 4)

        free = self.find_free_slots(
            start_day=start_day,
            end_day=end_day,
            duration_minutes=duration,
            limit=needed * 3,
        )
        # Spread across weeks: take up to `weekly` per week.
        chosen: list[tuple[datetime, datetime]] = []
        per_week: dict[str, int] = {}
        for slot in free:
            key = slot[0].strftime("%G-W%V")
            if per_week.get(key, 0) >= weekly:
                continue
            # Conflict / duplicate prevention against already chosen
            if any(_overlaps(slot[0], slot[1], c0, c1) for c0, c1 in chosen):
                continue
            # Duplicate prevention vs already scheduled for this goal at same start
            exists = self.conn.execute(
                """
                SELECT id FROM sessions
                WHERE goal_id = ? AND start_at = ? AND status IN ('scheduled', 'proposed', 'completed')
                """,
                (goal.id, slot[0].isoformat()),
            ).fetchone()
            if exists:
                continue
            chosen.append(slot)
            per_week[key] = per_week.get(key, 0) + 1
            if len(chosen) >= needed:
                break

        batch_id = _new_id()
        now = _now()
        sessions: list[SessionOut] = []
        title = goal.title
        for start, end in chosen:
            sid = _new_id()
            self.conn.execute(
                """
                INSERT INTO sessions (
                    id, goal_id, title, start_at, end_at, kind, status,
                    proposal_batch_id, notes, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, 'flexible', 'proposed', ?, NULL, ?, ?)
                """,
                (
                    sid,
                    goal.id,
                    title,
                    start.isoformat(),
                    end.isoformat(),
                    batch_id,
                    now,
                    now,
                ),
            )
            sessions.append(
                SessionOut(
                    id=sid,
                    goal_id=goal.id,
                    title=title,
                    start_at=start.isoformat(),
                    end_at=end.isoformat(),
                    kind="flexible",
                    status="proposed",
                    proposal_batch_id=batch_id,
                    notes=None,
                )
            )
        self.conn.commit()
        return sessions

    def approve_batch(self, batch_id: str) -> list[SessionOut]:
        rows = self.conn.execute(
            "SELECT * FROM sessions WHERE proposal_batch_id = ? AND status = 'proposed'",
            (batch_id,),
        ).fetchall()
        if not rows:
            return []
        now = _now()
        booked: list[SessionOut] = []
        for r in rows:
            # Duplicate prevention: skip if an identical scheduled session already exists
            dup = self.conn.execute(
                """
                SELECT id FROM sessions
                WHERE goal_id IS ? AND start_at = ? AND status = 'scheduled' AND id != ?
                """,
                (r["goal_id"], r["start_at"], r["id"]),
            ).fetchone()
            if dup:
                self.conn.execute(
                    "UPDATE sessions SET status='rejected', updated_at=? WHERE id=?",
                    (now, r["id"]),
                )
                continue
            self.conn.execute(
                "UPDATE sessions SET status='scheduled', kind='flexible', updated_at=? WHERE id=?",
                (now, r["id"]),
            )
            booked.append(self._row_to_session({**dict(r), "status": "scheduled"}))
        if booked and booked[0].goal_id:
            self.conn.execute(
                "UPDATE goals SET status='active', updated_at=? WHERE id=?",
                (now, booked[0].goal_id),
            )
        self.conn.commit()
        return booked

    def reject_batch(self, batch_id: str) -> int:
        now = _now()
        cur = self.conn.execute(
            """
            UPDATE sessions SET status='rejected', updated_at=?
            WHERE proposal_batch_id = ? AND status = 'proposed'
            """,
            (now, batch_id),
        )
        self.conn.commit()
        return cur.rowcount

    def mark_outcome(self, session_id: str, outcome: str, notes: str | None = None) -> SessionOut | None:
        if outcome not in {"completed", "missed"}:
            raise ValueError("outcome must be completed or missed")
        row = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()
        if not row:
            return None
        now = _now()
        self.conn.execute(
            "UPDATE sessions SET status=?, notes=COALESCE(?, notes), updated_at=? WHERE id=?",
            (outcome, notes, now, session_id),
        )
        self.conn.commit()
        refreshed = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()
        return self._row_to_session(refreshed)

    def find_session_for_outcome_phrase(self, text: str, goal_id: str | None = None) -> Any:
        """Best-effort match a scheduled session mentioned as completed/missed."""
        lower = text.lower()
        rows = self.conn.execute(
            """
            SELECT * FROM sessions
            WHERE status = 'scheduled'
              AND (? IS NULL OR goal_id = ?)
            ORDER BY start_at ASC
            """,
            (goal_id, goal_id),
        ).fetchall()
        if not rows:
            return None
        # Prefer earliest upcoming/past scheduled when user says completed/missed without date.
        if "missed" in lower:
            # prefer past-due first
            past = [r for r in rows if datetime.fromisoformat(r["start_at"]).date() <= datetime.now().date()]
            return past[0] if past else rows[0]
        return rows[0]

    def open_proposal_batch(self, goal_id: str) -> str | None:
        row = self.conn.execute(
            """
            SELECT proposal_batch_id FROM sessions
            WHERE goal_id = ? AND status = 'proposed'
            ORDER BY created_at DESC LIMIT 1
            """,
            (goal_id,),
        ).fetchone()
        return row["proposal_batch_id"] if row else None

    def progress_for_goal(self, goal_id: str) -> dict[str, int]:
        rows = self.conn.execute(
            """
            SELECT status, COUNT(*) AS c FROM sessions
            WHERE goal_id = ? AND status IN ('scheduled', 'completed', 'missed', 'proposed')
            GROUP BY status
            """,
            (goal_id,),
        ).fetchall()
        out = {"scheduled": 0, "completed": 0, "missed": 0, "proposed": 0}
        for r in rows:
            out[r["status"]] = r["c"]
        return out

    def _row_to_session(self, row) -> SessionOut:
        d = dict(row)
        return SessionOut(
            id=d["id"],
            goal_id=d.get("goal_id"),
            title=d["title"],
            start_at=d["start_at"],
            end_at=d["end_at"],
            kind=d["kind"],
            status=d["status"],
            proposal_batch_id=d.get("proposal_batch_id"),
            notes=d.get("notes"),
        )
