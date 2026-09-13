"""Deterministic proposal helpers — no AI."""

from __future__ import annotations

from collections import defaultdict
from datetime import datetime

from .calendar import CalendarService
from .schemas import Goal, SessionOut

_WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]


def _fmt_hm(minute: int) -> str:
    h, m = divmod(int(minute), 60)
    return f"{h}:{m:02d}"


def pattern_summary_from_goal(goal: Goal) -> str | None:
    plan = (goal.facts or {}).get("weekly_plan")
    if not isinstance(plan, dict):
        return None
    if isinstance(plan.get("pattern_summary"), str) and plan["pattern_summary"].strip():
        return plan["pattern_summary"].strip()
    slots = plan.get("slots") or []
    if not isinstance(slots, list) or not slots:
        return None
    parts: list[str] = []
    for slot in slots:
        if not isinstance(slot, dict):
            continue
        wd = slot.get("weekday")
        title = slot.get("title") or "Session"
        if not isinstance(wd, int) or not (0 <= wd <= 6):
            continue
        start_h = int(slot.get("start_hour", 17))
        start_m = int(slot.get("start_minute", 30))
        dur = int(slot.get("duration_minutes") or 60)
        start_min = start_h * 60 + start_m
        end_min = start_min + dur
        parts.append(f"{_WEEKDAYS[wd]} {title} {_fmt_hm(start_min)}–{_fmt_hm(end_min)}")
    return " · ".join(parts) if parts else None


def summarize_proposal(goal: Goal, sessions: list[SessionOut]) -> dict:
    """Compact approval card: weekly pattern + first-week sample + total count."""
    if not sessions:
        return {
            "pattern": None,
            "sample": [],
            "total": 0,
            "through": None,
            "text": "No sessions proposed.",
        }

    pattern = pattern_summary_from_goal(goal)
    by_week: dict[str, list[SessionOut]] = defaultdict(list)
    for s in sessions:
        start = datetime.fromisoformat(s.start_at)
        by_week[start.strftime("%G-W%V")].append(s)
    first_week_key = sorted(by_week.keys())[0]
    sample = by_week[first_week_key]
    last = max(sessions, key=lambda s: s.start_at)
    through = datetime.fromisoformat(last.end_at).date().isoformat()

    if not pattern:
        # Derive a short pattern from titles/weekdays in the sample week.
        bits = []
        for s in sample:
            start = datetime.fromisoformat(s.start_at)
            end = datetime.fromisoformat(s.end_at)
            bits.append(
                f"{_WEEKDAYS[start.weekday()]} {s.title} "
                f"{start.strftime('%H:%M')}–{end.strftime('%H:%M')}"
            )
        pattern = " · ".join(bits)

    text = (
        f"Weekly pattern: {pattern}\n"
        f"First week preview:\n"
        + "\n".join(
            f"• {datetime.fromisoformat(s.start_at).strftime('%a %d %b %H:%M')}–"
            f"{datetime.fromisoformat(s.end_at).strftime('%H:%M')} · {s.title}"
            for s in sample
        )
        + f"\nThen repeats through {through} · {len(sessions)} sessions total. "
        "Nothing is booked until you approve."
    )
    return {
        "pattern": pattern,
        "sample": sample,
        "total": len(sessions),
        "through": through,
        "text": text,
    }


def propose_for_goal(calendar: CalendarService, goal: Goal) -> tuple[str, list[SessionOut], dict]:
    sessions = calendar.propose_goal_sessions(goal)
    if not sessions:
        empty = {
            "pattern": None,
            "sample": [],
            "total": 0,
            "through": None,
            "text": (
                "I checked your calendar and couldn't find free slots that fit right now. "
                "We can adjust days or times, then try again — nothing was booked."
            ),
        }
        return empty["text"], [], empty

    if goal.status in {"gathering", "ready_to_plan"}:
        goal.status = "planned"

    summary = summarize_proposal(goal, sessions)
    return summary["text"], sessions, summary
