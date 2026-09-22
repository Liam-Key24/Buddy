"""Buddy calendar: availability, dated proposals, approval, and booking."""

from __future__ import annotations

import re
import uuid
from datetime import date, datetime, timedelta, timezone
from typing import Any

from .categories import CategoryStore
from .schemas import CalendarAction, CategoryBrief, SessionOut


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


_CLOCK_RE = re.compile(
    r"\b\d{1,2}(?::\d{2})?\s*(?:a\.?m\.?|p\.?m\.?)\b|\b\d{1,2}:\d{2}\b",
    re.I,
)
_ONE_OFF_FREQ = {
    "once",
    "one",
    "one-off",
    "one off",
    "single",
    "single event",
    "one time",
    "one-time",
}


def frequency_to_weekly_count(frequency: str | None, commitment: str | None = None) -> int:
    raw = f"{commitment or ''} {frequency or ''}".strip()
    if not raw:
        return 2
    # "8 pm" / "8pm" / "20:00" are clock times, never session counts.
    text = _CLOCK_RE.sub(" ", raw.lower())
    text = re.sub(r"\s+", " ", text).strip()
    if not text or text in {"at", "around", "about", "by"}:
        return 1
    if text in _ONE_OFF_FREQ or re.search(
        r"\b(single event|one-?off|once only|just once)\b", text
    ):
        return 1
    if "every day" in text or "daily" in text:
        return 7
    # Ranges like 3-4 → prefer the higher end when planning capacity
    m_range = re.search(r"(\d+)\s*[-–]\s*(\d+)", text)
    if m_range:
        return max(1, int(m_range.group(2)))
    m = re.search(r"(\d+)\s*(?:x|×|times?)\s*(?:a|per)?\s*week", text)
    if m:
        return max(1, int(m.group(1)))
    m = re.search(r"(\d+)\s+per\s+week", text)
    if m:
        return max(1, int(m.group(1)))
    if "twice" in text or "two times" in text:
        return 2
    if re.search(r"\bonce\s+(a|per)\s+(week|month|day)", text):
        return 1
    if "once" in text or "one time" in text:
        return 1
    if "four" in text:
        return 4
    if "three" in text:
        return 3
    return 2


def is_one_off_goal(goal: Any, plan: dict[str, Any] | None = None) -> bool:
    """True for a single dated event — not a repeating weekly plan."""
    if plan is None:
        facts = getattr(goal, "facts", None) or {}
        raw = facts.get("weekly_plan") if isinstance(facts, dict) else None
        plan = raw if isinstance(raw, dict) else None
    freq = (getattr(goal, "frequency", None) or "").strip().lower()
    commit = (getattr(goal, "commitment", None) or "").strip().lower()
    blob = f"{freq} {commit}".strip()
    if freq in _ONE_OFF_FREQ or commit in _ONE_OFF_FREQ:
        return True
    if re.search(r"\b(single event|one-?off|once only|just once)\b", blob):
        return True
    if freq and not _CLOCK_RE.sub(" ", freq).strip():
        return True
    if not isinstance(plan, dict):
        return False
    repeat = str(plan.get("repeat") or "").lower().replace("-", "_").replace(" ", "_")
    if repeat in {"once", "one", "one_off", "single"}:
        return True
    try:
        if int(plan.get("occurrences") or 0) == 1:
            return True
    except (TypeError, ValueError):
        pass
    return bool(plan.get("on_date"))


def _plan_on_date(plan: dict[str, Any] | None) -> date | None:
    if not isinstance(plan, dict) or not plan.get("on_date"):
        return None
    try:
        return date.fromisoformat(str(plan["on_date"])[:10])
    except ValueError:
        return None


def session_duration_minutes(domain: str | None) -> int:
    if domain == "climbing":
        return 90
    if domain == "reading":
        return 45
    if domain == "savings":
        return 30
    if domain == "product":
        return 90
    return 60


def _month_end(year: int, month: int) -> date:
    if month == 12:
        return date(year, 12, 31)
    return date(year, month + 1, 1) - timedelta(days=1)


def _parse_deadline_end(deadline: str | None) -> date:
    """Parse goal deadline to an inclusive end date.

    Month-only values use the last day of that month. If the model (or user text)
    yields a past year for a month name like "November", roll forward year-by-year
    until the deadline is today or later so proposals still land on the calendar.
    """
    today = datetime.now().date()
    if not deadline:
        return today + timedelta(weeks=8)
    try:
        parts = [int(p) for p in deadline.strip().split("-")[:3]]
        if len(parts) < 2:
            return today + timedelta(weeks=8)
        year, month = parts[0], parts[1]
        if not (1 <= month <= 12):
            return today + timedelta(weeks=8)
        if len(parts) >= 3:
            day = parts[2]
            try:
                end = date(year, month, day)
            except ValueError:
                end = _month_end(year, month)
        else:
            end = _month_end(year, month)
        # Roll past deadlines forward (common when AI omits/wrong-years "end of November").
        guard = 0
        while end < today and guard < 6:
            year += 1
            if len(parts) >= 3:
                try:
                    end = date(year, month, parts[2])
                except ValueError:
                    end = _month_end(year, month)
            else:
                end = _month_end(year, month)
            guard += 1
        if end < today:
            return today + timedelta(weeks=8)
        return end
    except Exception:
        return today + timedelta(weeks=8)


def _overlaps(a_start: datetime, a_end: datetime, b_start: datetime, b_end: datetime) -> bool:
    return a_start < b_end and b_start < a_end


class CalendarService:
    def __init__(self, conn, categories: CategoryStore | None = None):
        self.conn = conn
        self.categories = categories or CategoryStore(conn)

    def list_sessions(
        self,
        *,
        start: str | None = None,
        end: str | None = None,
        statuses: list[str] | None = None,
    ) -> list[SessionOut]:
        self.categories.ensure_defaults()
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
        sessions = [self._row_to_session(r) for r in rows]
        dirty = False
        for i, s in enumerate(sessions):
            if s.category_id:
                continue
            cid = self.categories.match_title(s.title)
            if not cid:
                continue
            self.conn.execute("UPDATE sessions SET category_id=? WHERE id=?", (cid, s.id))
            dirty = True
            sessions[i] = self._row_to_session(
                {**dict(rows[i]), "category_id": cid}
            )
        if dirty:
            self.conn.commit()
        return sessions

    def list_fixed_blocks(self) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            "SELECT * FROM fixed_blocks ORDER BY weekday, start_minute"
        ).fetchall()
        return [dict(r) for r in rows]

    def create_fixed_block(
        self,
        *,
        title: str,
        weekday: int,
        start_minute: int,
        end_minute: int,
    ) -> dict[str, Any]:
        title = title.strip()
        if not title:
            raise ValueError("Title required")
        if not (0 <= weekday <= 6):
            raise ValueError("weekday must be 0–6 (Mon–Sun)")
        if not (0 <= start_minute < end_minute <= 24 * 60):
            raise ValueError("Invalid start/end minutes")
        bid = _new_id()
        self.conn.execute(
            """
            INSERT INTO fixed_blocks (id, title, weekday, start_minute, end_minute, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            (bid, title, weekday, start_minute, end_minute, _now()),
        )
        self.conn.commit()
        row = self.conn.execute("SELECT * FROM fixed_blocks WHERE id = ?", (bid,)).fetchone()
        return dict(row)

    def update_fixed_block(
        self,
        block_id: str,
        *,
        title: str | None = None,
        weekday: int | None = None,
        start_minute: int | None = None,
        end_minute: int | None = None,
    ) -> dict[str, Any] | None:
        row = self.conn.execute("SELECT * FROM fixed_blocks WHERE id = ?", (block_id,)).fetchone()
        if not row:
            return None
        data = dict(row)
        if title is not None:
            title = title.strip()
            if not title:
                raise ValueError("Title required")
            data["title"] = title
        if weekday is not None:
            if not (0 <= weekday <= 6):
                raise ValueError("weekday must be 0–6 (Mon–Sun)")
            data["weekday"] = weekday
        if start_minute is not None:
            data["start_minute"] = start_minute
        if end_minute is not None:
            data["end_minute"] = end_minute
        if not (0 <= data["start_minute"] < data["end_minute"] <= 24 * 60):
            raise ValueError("Invalid start/end minutes")
        self.conn.execute(
            """
            UPDATE fixed_blocks
            SET title=?, weekday=?, start_minute=?, end_minute=?
            WHERE id=?
            """,
            (
                data["title"],
                data["weekday"],
                data["start_minute"],
                data["end_minute"],
                block_id,
            ),
        )
        self.conn.commit()
        return data

    def delete_fixed_block(self, block_id: str) -> bool:
        cur = self.conn.execute("DELETE FROM fixed_blocks WHERE id = ?", (block_id,))
        self.conn.commit()
        return cur.rowcount > 0

    def list_proposed_for_batch(self, batch_id: str) -> list[SessionOut]:
        rows = self.conn.execute(
            """
            SELECT * FROM sessions
            WHERE proposal_batch_id = ? AND status = 'proposed'
            ORDER BY start_at ASC
            """,
            (batch_id,),
        ).fetchall()
        return [self._row_to_session(r) for r in rows]

    def _category_for_title(self, title: str) -> tuple[str | None, CategoryBrief | None]:
        cat = self.categories.classify_session_title(title)
        if not cat:
            return None, None
        brief = CategoryBrief(
            id=cat["id"],
            name=cat["name"],
            color=cat["color"],
            icon=cat.get("icon") or "circle",
            keywords=cat.get("keywords") or "",
        )
        return cat["id"], brief

    def reclassify_all(self) -> int:
        """Re-run title matching for every session. Returns updated count."""
        self.categories.ensure_defaults()
        rows = self.conn.execute("SELECT id, title FROM sessions").fetchall()
        n = 0
        for r in rows:
            cid = self.categories.match_title(r["title"])
            self.conn.execute("UPDATE sessions SET category_id=? WHERE id=?", (cid, r["id"]))
            n += 1
        self.conn.commit()
        return n

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

    def _slot_on_day(
        self,
        day: date,
        *,
        duration_minutes: int,
        prefer_hour: int,
        prefer_minute: int,
        window_start_hour: int = 17,
        window_end_hour: int = 21,
        extra_busy: list[tuple[datetime, datetime]] | None = None,
    ) -> tuple[datetime, datetime] | None:
        """Place one session on a day near the preferred time, within the evening window."""
        busy = list(self._busy_intervals(day))
        if extra_busy:
            busy.extend(extra_busy)
        preferred = datetime(day.year, day.month, day.day, prefer_hour, prefer_minute)
        window_start = datetime(day.year, day.month, day.day, window_start_hour, 0)
        window_end = datetime(day.year, day.month, day.day, window_end_hour, 0)
        # Try preferred, then step forward, then earlier within the window.
        candidates: list[datetime] = [preferred]
        cursor = preferred + timedelta(minutes=30)
        while cursor + timedelta(minutes=duration_minutes) <= window_end:
            candidates.append(cursor)
            cursor += timedelta(minutes=30)
        cursor = preferred - timedelta(minutes=30)
        while cursor >= window_start:
            candidates.append(cursor)
            cursor -= timedelta(minutes=30)
        for start in candidates:
            end = start + timedelta(minutes=duration_minutes)
            if start < window_start or end > window_end:
                continue
            if day == datetime.now().date() and start < datetime.now() - timedelta(minutes=2):
                continue
            if any(_overlaps(start, end, b0, b1) for b0, b1 in busy):
                continue
            return start, end
        return None

    def _missed_count(self, goal_id: str) -> int:
        row = self.conn.execute(
            "SELECT COUNT(*) AS c FROM sessions WHERE goal_id = ? AND status = 'missed'",
            (goal_id,),
        ).fetchone()
        return int(row["c"] if row else 0)

    def _append_catch_up(
        self,
        goal: Any,
        chosen: list[tuple[datetime, datetime, str]],
        *,
        start_day: date,
        end_day: date,
        duration_minutes: int,
        prefer_hour: int = 17,
        prefer_minute: int = 30,
        window_start_hour: int = 17,
        window_end_hour: int = 21,
        avoid: set[int] | None = None,
        title_pool: list[str] | None = None,
        limit: int | None = None,
    ) -> int:
        """Stack makeup proposals for missed sessions into free slots before the deadline.

        Prefers empty days first, then stacks later on days that already have a session.
        Returns how many catch-up slots were added (titles get a · Catch-up suffix).
        """
        needed = self._missed_count(goal.id) if limit is None else max(0, int(limit))
        if needed <= 0:
            return 0
        avoid = avoid or set()
        titles = [t.strip() for t in (title_pool or []) if isinstance(t, str) and t.strip()]
        if not titles:
            titles = [str(getattr(goal, "title", None) or "Session")]

        def catch_title(i: int) -> str:
            base = titles[i % len(titles)]
            if "catch-up" in base.lower():
                return base
            return f"{base} · Catch-up"

        added = 0
        days_with = {c[0].date() for c in chosen}

        for prefer_empty in (True, False):
            day = start_day
            while day <= end_day and added < needed:
                if day.weekday() in avoid:
                    day += timedelta(days=1)
                    continue
                if prefer_empty and day in days_with:
                    day += timedelta(days=1)
                    continue
                if not prefer_empty and day not in days_with:
                    day += timedelta(days=1)
                    continue

                extra = [(c0, c1) for c0, c1, _t in chosen if c0.date() == day]
                # On occupied days, prefer later in the window so catch-ups stack after the plan slot.
                hour = prefer_hour
                minute = prefer_minute
                if not prefer_empty and extra:
                    latest = max(c1 for _c0, c1 in extra)
                    hour = latest.hour
                    minute = latest.minute
                    if minute % 30:
                        minute = 30 if minute < 30 else 0
                        if minute == 0:
                            hour += 1

                placed = self._slot_on_day(
                    day,
                    duration_minutes=duration_minutes,
                    prefer_hour=min(hour, max(0, window_end_hour - 1)),
                    prefer_minute=minute if minute in (0, 30) else 30,
                    window_start_hour=min(window_start_hour, prefer_hour),
                    window_end_hour=window_end_hour,
                    extra_busy=extra,
                )
                if placed:
                    start, end = placed
                    exists = self.conn.execute(
                        """
                        SELECT id FROM sessions
                        WHERE goal_id = ? AND start_at = ?
                          AND status IN ('scheduled', 'proposed', 'completed')
                        """,
                        (goal.id, start.isoformat()),
                    ).fetchone()
                    if not exists and not any(
                        _overlaps(start, end, c0, c1) for c0, c1, _t in chosen
                    ):
                        chosen.append((start, end, catch_title(added)))
                        days_with.add(day)
                        added += 1
                day += timedelta(days=1)

        return added

    def propose_goal_sessions(self, goal: Any, *, weeks: int | None = None) -> list[SessionOut]:
        """Create dated proposed sessions from real availability. Replaces open proposals for the goal."""
        self.conn.execute(
            """
            DELETE FROM sessions
            WHERE goal_id = ? AND status = 'proposed'
            """,
            (goal.id,),
        )

        today = datetime.now().date()
        plan = (getattr(goal, "facts", None) or {}).get("weekly_plan")
        plan = plan if isinstance(plan, dict) else None
        one_off = is_one_off_goal(goal, plan)
        on_date = _plan_on_date(plan)

        if one_off:
            start_day = on_date or today
            if start_day < today:
                start_day = today
            end_day = start_day
        else:
            start_day = today + timedelta(days=1)
            end_day = _parse_deadline_end(goal.deadline)
            if end_day < start_day:
                end_day = start_day + timedelta(weeks=8)
            if weeks:
                end_day = min(end_day, today + timedelta(weeks=weeks))

        if plan and isinstance(plan.get("slots"), list) and plan["slots"]:
            sessions = self._propose_from_weekly_plan(goal, plan, start_day, end_day)
            self.conn.commit()
            return sessions

        # Generic path (no structured weekly plan): keep a short approval horizon.
        if one_off:
            weekly = 1
            base_needed = 1
        else:
            end_day = min(end_day, today + timedelta(weeks=4))
            weekly = frequency_to_weekly_count(goal.frequency, goal.commitment)
            base_needed = weekly * max(1, ((end_day - start_day).days // 7) + 1)
            base_needed = min(base_needed, weekly * 4)
        duration = session_duration_minutes(goal.domain)
        missed = self._missed_count(goal.id)

        prefer_after = 17
        facts = getattr(goal, "facts", None) or {}
        if isinstance(facts.get("prefer_after_hour"), int):
            prefer_after = max(0, min(22, int(facts["prefer_after_hour"])))

        free = self.find_free_slots(
            start_day=start_day,
            end_day=end_day,
            duration_minutes=duration,
            prefer_hours=(prefer_after, 21),
            limit=max(base_needed, 1) * 3,
        )
        avoid = set()
        if isinstance(facts.get("avoid_weekdays"), list):
            avoid = {int(x) for x in facts["avoid_weekdays"] if isinstance(x, int)}

        chosen: list[tuple[datetime, datetime, str]] = []
        per_week: dict[str, int] = {}
        for slot in free:
            if len(chosen) >= base_needed:
                break
            if slot[0].weekday() in avoid:
                continue
            key = slot[0].strftime("%G-W%V")
            if per_week.get(key, 0) >= weekly:
                continue
            if any(_overlaps(slot[0], slot[1], c0, c1) for c0, c1, _t in chosen):
                continue
            exists = self.conn.execute(
                """
                SELECT id FROM sessions
                WHERE goal_id = ? AND start_at = ? AND status IN ('scheduled', 'proposed', 'completed')
                """,
                (goal.id, slot[0].isoformat()),
            ).fetchone()
            if exists:
                continue
            chosen.append((slot[0], slot[1], goal.title))
            per_week[key] = per_week.get(key, 0) + 1

        if missed and not one_off:
            self._append_catch_up(
                goal,
                chosen,
                start_day=start_day,
                end_day=end_day,
                duration_minutes=duration,
                prefer_hour=prefer_after,
                prefer_minute=30,
                window_start_hour=prefer_after,
                window_end_hour=21,
                avoid=avoid,
                title_pool=[goal.title],
                limit=missed,
            )

        return self._insert_proposed_titled(goal, chosen)

    def _propose_from_weekly_plan(
        self,
        goal: Any,
        plan: dict[str, Any],
        start_day: date,
        end_day: date,
    ) -> list[SessionOut]:
        slots = [s for s in plan.get("slots", []) if isinstance(s, dict)]
        avoid = set()
        if isinstance(plan.get("avoid_weekdays"), list):
            avoid = {int(x) for x in plan["avoid_weekdays"] if isinstance(x, int)}
        window_start = int(plan.get("prefer_after_hour", 17))
        window_end = int(plan.get("window_end_hour", 21))
        one_off = is_one_off_goal(goal, plan)

        chosen: list[tuple[datetime, datetime, str]] = []
        if one_off:
            specs = slots or [
                {
                    "title": goal.title,
                    "start_hour": window_start,
                    "start_minute": 0,
                    "duration_minutes": session_duration_minutes(goal.domain),
                }
            ]
            for spec in specs:
                title = str(spec.get("title") or goal.title).strip() or goal.title
                duration = int(spec.get("duration_minutes") or session_duration_minutes(goal.domain))
                prefer_hour = int(spec.get("start_hour", window_start))
                prefer_minute = int(spec.get("start_minute", 0))
                end_hour = min(23, max(window_end, prefer_hour + 1))
                placed = self._slot_on_day(
                    start_day,
                    duration_minutes=duration,
                    prefer_hour=prefer_hour,
                    prefer_minute=prefer_minute,
                    window_start_hour=min(window_start, prefer_hour),
                    window_end_hour=end_hour,
                )
                if not placed and start_day == datetime.now().date():
                    placed = self._slot_on_day(
                        start_day + timedelta(days=1),
                        duration_minutes=duration,
                        prefer_hour=prefer_hour,
                        prefer_minute=prefer_minute,
                        window_start_hour=min(window_start, prefer_hour),
                        window_end_hour=end_hour,
                    )
                if placed:
                    chosen.append((placed[0], placed[1], title))
            return self._insert_proposed_titled(goal, chosen, commit=False)

        day = start_day
        while day <= end_day:
            if day.weekday() in avoid:
                day += timedelta(days=1)
                continue
            for spec in slots:
                wd = spec.get("weekday")
                if not isinstance(wd, int) or wd != day.weekday():
                    continue
                title = str(spec.get("title") or goal.title).strip() or goal.title
                duration = int(spec.get("duration_minutes") or session_duration_minutes(goal.domain))
                prefer_hour = int(spec.get("start_hour", window_start))
                prefer_minute = int(spec.get("start_minute", 30))
                placed = self._slot_on_day(
                    day,
                    duration_minutes=duration,
                    prefer_hour=prefer_hour,
                    prefer_minute=prefer_minute,
                    window_start_hour=min(window_start, prefer_hour),
                    window_end_hour=window_end,
                )
                if not placed:
                    continue
                start, end = placed
                exists = self.conn.execute(
                    """
                    SELECT id FROM sessions
                    WHERE goal_id = ? AND start_at = ? AND status IN ('scheduled', 'proposed', 'completed')
                    """,
                    (goal.id, start.isoformat()),
                ).fetchone()
                if exists:
                    continue
                if any(_overlaps(start, end, c0, c1) for c0, c1, _t in chosen):
                    continue
                chosen.append((start, end, title))
            day += timedelta(days=1)

        title_pool = [
            str(s.get("title") or goal.title).strip() or goal.title
            for s in slots
            if isinstance(s, dict)
        ]
        default_dur = session_duration_minutes(goal.domain)
        if slots:
            default_dur = int(slots[0].get("duration_minutes") or default_dur)
        self._append_catch_up(
            goal,
            chosen,
            start_day=start_day,
            end_day=end_day,
            duration_minutes=default_dur,
            prefer_hour=window_start,
            prefer_minute=30,
            window_start_hour=window_start,
            window_end_hour=window_end,
            avoid=avoid,
            title_pool=title_pool or [goal.title],
        )

        return self._insert_proposed_titled(goal, chosen, commit=False)

    def _insert_proposed_titled(
        self,
        goal: Any,
        chosen: list[tuple[datetime, datetime, str]],
        *,
        commit: bool = True,
    ) -> list[SessionOut]:
        batch_id = _new_id()
        now = _now()
        sessions: list[SessionOut] = []
        for start, end, title in chosen:
            sid = _new_id()
            notes = "catch-up" if "catch-up" in title.lower() else None
            category_id, category = self._category_for_title(title)
            self.conn.execute(
                """
                INSERT INTO sessions (
                    id, goal_id, title, start_at, end_at, kind, status,
                    proposal_batch_id, notes, created_at, updated_at, category_id
                ) VALUES (?, ?, ?, ?, ?, 'flexible', 'proposed', ?, ?, ?, ?, ?)
                """,
                (
                    sid,
                    goal.id,
                    title,
                    start.isoformat(),
                    end.isoformat(),
                    batch_id,
                    notes,
                    now,
                    now,
                    category_id,
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
                    notes=notes,
                    category_id=category_id,
                    category=category,
                )
            )
        if commit:
            self.conn.commit()
        return sessions

    def _insert_proposed(
        self,
        goal: Any,
        chosen: list[tuple[datetime, datetime]],
        *,
        title_for,
    ) -> list[SessionOut]:
        titled = [(start, end, title_for(start, end)) for start, end in chosen]
        return self._insert_proposed_titled(goal, titled)

    def approve_batch(self, batch_id: str, *, conversation_id: str | None = None) -> list[SessionOut]:
        rows = self.conn.execute(
            "SELECT * FROM sessions WHERE proposal_batch_id = ? AND status = 'proposed'",
            (batch_id,),
        ).fetchall()
        if not rows:
            return []
        now = _now()
        booked: list[SessionOut] = []
        session_ids: list[str] = []
        for r in rows:
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
            session_ids.append(r["id"])
        if booked and booked[0].goal_id:
            self.conn.execute(
                "UPDATE goals SET status='active', updated_at=? WHERE id=?",
                (now, booked[0].goal_id),
            )
        if booked:
            import json

            self.conn.execute(
                """
                INSERT INTO approval_events (
                    id, batch_id, goal_id, conversation_id, approved_at, undone_at, session_ids_json
                ) VALUES (?, ?, ?, ?, ?, NULL, ?)
                """,
                (
                    _new_id(),
                    batch_id,
                    booked[0].goal_id,
                    conversation_id,
                    now,
                    json.dumps(session_ids),
                ),
            )
        self.conn.commit()
        return booked

    def undo_batch(self, batch_id: str) -> list[SessionOut]:
        """Revert an approved booking without AI. Sessions return to proposed."""
        import json

        event = self.conn.execute(
            """
            SELECT * FROM approval_events
            WHERE batch_id = ? AND undone_at IS NULL
            ORDER BY approved_at DESC LIMIT 1
            """,
            (batch_id,),
        ).fetchone()
        now = _now()
        if event:
            ids = json.loads(event["session_ids_json"] or "[]")
            restored: list[SessionOut] = []
            for sid in ids:
                row = self.conn.execute(
                    "SELECT * FROM sessions WHERE id = ? AND status = 'scheduled'",
                    (sid,),
                ).fetchone()
                if not row:
                    continue
                self.conn.execute(
                    "UPDATE sessions SET status='proposed', updated_at=? WHERE id=?",
                    (now, sid),
                )
                restored.append(self._row_to_session({**dict(row), "status": "proposed"}))
            self.conn.execute(
                "UPDATE approval_events SET undone_at=? WHERE id=?",
                (now, event["id"]),
            )
            if restored and restored[0].goal_id:
                self.conn.execute(
                    "UPDATE goals SET status='planned', updated_at=? WHERE id=?",
                    (now, restored[0].goal_id),
                )
            self.conn.commit()
            return restored

        # Fallback: any scheduled rows for this batch
        rows = self.conn.execute(
            "SELECT * FROM sessions WHERE proposal_batch_id = ? AND status = 'scheduled'",
            (batch_id,),
        ).fetchall()
        restored = []
        for r in rows:
            self.conn.execute(
                "UPDATE sessions SET status='proposed', updated_at=? WHERE id=?",
                (now, r["id"]),
            )
            restored.append(self._row_to_session({**dict(r), "status": "proposed"}))
        self.conn.commit()
        return restored

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

    def span_for_goal(self, goal_id: str) -> dict[str, str | None]:
        row = self.conn.execute(
            """
            SELECT MIN(start_at) AS started_at,
                   MAX(COALESCE(end_at, start_at)) AS ended_at
            FROM sessions
            WHERE goal_id = ? AND status IN ('scheduled', 'completed', 'missed')
            """,
            (goal_id,),
        ).fetchone()
        if not row:
            return {"started_at": None, "ended_at": None}
        return {
            "started_at": row["started_at"],
            "ended_at": row["ended_at"],
        }

    def create_manual_session(
        self,
        *,
        title: str,
        start_at: str,
        end_at: str,
        category_id: str | None = None,
    ) -> SessionOut:
        """Create a scheduled session without Cloud AI."""
        title = title.strip()
        if not title:
            raise ValueError("Title required")
        try:
            start = datetime.fromisoformat(start_at.replace("Z", "+00:00"))
            end = datetime.fromisoformat(end_at.replace("Z", "+00:00"))
        except Exception as exc:
            raise ValueError("Invalid start/end time") from exc
        if end <= start:
            raise ValueError("End must be after start")

        if category_id and not self.categories.get(category_id):
            category_id = None
        if not category_id:
            category_id, _ = self._category_for_title(title)

        sid = _new_id()
        now = _now()
        self.conn.execute(
            """
            INSERT INTO sessions (
                id, goal_id, title, start_at, end_at, kind, status,
                proposal_batch_id, notes, created_at, updated_at, category_id
            ) VALUES (?, NULL, ?, ?, ?, 'flexible', 'scheduled', NULL, NULL, ?, ?, ?)
            """,
            (sid, title, start.isoformat(), end.isoformat(), now, now, category_id),
        )
        self.conn.commit()
        row = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (sid,)).fetchone()
        return self._row_to_session(row)

    def get_session(self, session_id: str) -> SessionOut | None:
        row = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()
        if not row:
            return None
        return self._row_to_session(row)

    def resolve_action_targets(self, action: CalendarAction) -> list[SessionOut]:
        if action.session_id:
            session = self.get_session(action.session_id)
            return [session] if session else []

        sessions = self.list_sessions(
            statuses=list(action.statuses) if action.statuses else None,
        )
        matches: list[SessionOut] = []
        for session in sessions:
            if action.goal_id and session.goal_id != action.goal_id:
                continue
            if action.title_contains and action.title_contains.lower() not in session.title.lower():
                continue
            if action.date and not session.start_at.startswith(action.date):
                continue
            matches.append(session)

        if not matches:
            return []
        if action.all_matching:
            return matches
        return [matches[0]]

    def update_session(
        self,
        session_id: str,
        *,
        title: str | None = None,
        start_at: str | None = None,
        end_at: str | None = None,
        category_id: str | None | object = ...,
    ) -> SessionOut | None:
        row = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()
        if not row:
            return None
        current = dict(row)
        new_title = title.strip() if title else current["title"]
        new_start = start_at or current["start_at"]
        new_end = end_at or current["end_at"]
        try:
            start = datetime.fromisoformat(new_start.replace("Z", "+00:00"))
            end = datetime.fromisoformat(new_end.replace("Z", "+00:00"))
        except Exception as exc:
            raise ValueError("Invalid start/end time") from exc
        if end <= start:
            raise ValueError("End must be after start")

        if category_id is not ...:
            new_category_id = category_id or None
            if new_category_id and not self.categories.get(str(new_category_id)):
                new_category_id = None
        else:
            new_category_id = current.get("category_id")
            if title and category_id is ...:
                matched, _ = self._category_for_title(new_title)
                if matched:
                    new_category_id = matched

        now = _now()
        self.conn.execute(
            """
            UPDATE sessions
            SET title=?, start_at=?, end_at=?, category_id=?, updated_at=?
            WHERE id=?
            """,
            (new_title, start.isoformat(), end.isoformat(), new_category_id, now, session_id),
        )
        self.conn.commit()
        updated = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()
        return self._row_to_session(updated)

    def move_session(
        self,
        session_id: str,
        *,
        new_start_at: str,
        new_end_at: str | None = None,
    ) -> SessionOut | None:
        row = self.conn.execute("SELECT * FROM sessions WHERE id = ?", (session_id,)).fetchone()
        if not row:
            return None
        current = dict(row)
        start = datetime.fromisoformat(new_start_at.replace("Z", "+00:00"))
        if new_end_at:
            end = datetime.fromisoformat(new_end_at.replace("Z", "+00:00"))
        else:
            old_start = datetime.fromisoformat(current["start_at"].replace("Z", "+00:00"))
            old_end = datetime.fromisoformat(current["end_at"].replace("Z", "+00:00"))
            duration = old_end - old_start
            end = start + duration
        return self.update_session(
            session_id,
            start_at=start.isoformat(),
            end_at=end.isoformat(),
        )

    def apply_calendar_action(self, action: CalendarAction) -> tuple[list[str], list[SessionOut], list[str]]:
        deleted: list[str] = []
        updated: list[SessionOut] = []
        errors: list[str] = []
        targets = self.resolve_action_targets(action)
        if not targets:
            errors.append("No matching calendar session found")
            return deleted, updated, errors

        for session in targets:
            try:
                if action.op == "delete":
                    if self.delete_session(session.id):
                        deleted.append(session.id)
                elif action.op == "update":
                    result = self.update_session(
                        session.id,
                        title=action.new_title,
                        start_at=action.new_start_at,
                        end_at=action.new_end_at,
                    )
                    if result:
                        updated.append(result)
                elif action.op == "move":
                    if not action.new_start_at:
                        errors.append(f"Move for {session.title} needs new_start_at")
                        continue
                    result = self.move_session(
                        session.id,
                        new_start_at=action.new_start_at,
                        new_end_at=action.new_end_at,
                    )
                    if result:
                        updated.append(result)
                elif action.op == "mark_outcome":
                    if not action.outcome:
                        errors.append(f"Outcome for {session.title} needs completed or missed")
                        continue
                    result = self.mark_outcome(session.id, action.outcome, action.notes)
                    if result:
                        updated.append(result)
            except ValueError as exc:
                errors.append(str(exc))
        return deleted, updated, errors

    def delete_session(self, session_id: str) -> bool:
        """Hard-delete a session by id. Returns False if missing."""
        row = self.conn.execute("SELECT id FROM sessions WHERE id = ?", (session_id,)).fetchone()
        if not row:
            return False
        self.conn.execute("DELETE FROM sessions WHERE id = ?", (session_id,))
        self.conn.commit()
        return True

    def _row_to_session(self, row) -> SessionOut:
        d = dict(row)
        category_id = d.get("category_id")
        category = None
        if category_id:
            cat = self.categories.get(category_id)
            if cat:
                category = CategoryBrief(
                    id=cat["id"],
                    name=cat["name"],
                    color=cat["color"],
                    icon=cat.get("icon") or "circle",
                    keywords=cat.get("keywords") or "",
                )
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
            category_id=category_id,
            category=category,
        )
