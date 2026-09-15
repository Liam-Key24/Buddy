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


def summarize_proposal(goal: Goal, sessions: list[SessionOut], *, why: dict | None = None) -> dict:
    """Compact approval card: weekly pattern + first-week sample + total count."""
    if not sessions:
        return {
            "pattern": None,
            "sample": [],
            "total": 0,
            "through": None,
            "text": "No sessions proposed.",
            "why": why or {},
            "goal_card": _goal_card(goal),
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
        bits = []
        for s in sample:
            start = datetime.fromisoformat(s.start_at)
            end = datetime.fromisoformat(s.end_at)
            bits.append(
                f"{_WEEKDAYS[start.weekday()]} {s.title} "
                f"{start.strftime('%H:%M')}–{end.strftime('%H:%M')}"
            )
        pattern = " · ".join(bits)

    why = why or {}
    why_lines = []
    if why.get("fixed_blocks_reviewed") is not None:
        why_lines.append(f"Reviewed {why['fixed_blocks_reviewed']} fixed calendar blocks")
    if why.get("conflicts_avoided") is not None:
        why_lines.append(f"Avoided {why['conflicts_avoided']} busy conflicts")
    if why.get("prefer_after_hour") is not None:
        why_lines.append(f"Placed sessions after {why['prefer_after_hour']}:00 as requested")
    if why.get("avoid_weekdays"):
        names = [_WEEKDAYS[i] for i in why["avoid_weekdays"] if isinstance(i, int) and 0 <= i <= 6]
        if names:
            why_lines.append(f"Kept {', '.join(names)} clear")
    if why.get("preference_note"):
        why_lines.append(str(why["preference_note"]))

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
        "why": why,
        "why_lines": why_lines,
        "goal_card": _goal_card(goal),
    }


def _goal_card(goal: Goal) -> dict:
    plan = (goal.facts or {}).get("weekly_plan") if goal.facts else None
    strategy = None
    if isinstance(plan, dict):
        strategy = plan.get("pattern_summary")
    return {
        "title": goal.title,
        "outcome": goal.target,
        "baseline": goal.baseline,
        "deadline": goal.deadline,
        "frequency": goal.frequency,
        "strategy": strategy,
        "realistic_note": (
            "Built from your cadence and real calendar availability — nothing is booked yet."
        ),
    }


def propose_for_goal(calendar: CalendarService, goal: Goal) -> tuple[str, list[SessionOut], dict]:
    fixed_count = len(calendar.list_fixed_blocks())
    plan = (goal.facts or {}).get("weekly_plan") if goal.facts else None
    prefer_after = 17
    avoid: list[int] = []
    if isinstance(plan, dict):
        prefer_after = int(plan.get("prefer_after_hour", 17))
        if isinstance(plan.get("avoid_weekdays"), list):
            avoid = [int(x) for x in plan["avoid_weekdays"] if isinstance(x, int)]

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
            "why": {"fixed_blocks_reviewed": fixed_count, "conflicts_avoided": 0},
            "why_lines": [f"Reviewed {fixed_count} fixed calendar blocks", "No calendar changes made"],
            "goal_card": _goal_card(goal),
        }
        return empty["text"], [], empty

    if goal.status in {"gathering", "ready_to_plan"}:
        goal.status = "planned"

    why = {
        "fixed_blocks_reviewed": fixed_count,
        "conflicts_avoided": fixed_count,  # each fixed block is a protected commitment
        "prefer_after_hour": prefer_after,
        "avoid_weekdays": avoid,
        "sessions_found": len(sessions),
    }
    summary = summarize_proposal(goal, sessions, why=why)
    return summary["text"], sessions, summary
