"""Deterministic proposal helpers — no AI."""

from __future__ import annotations

from .calendar import CalendarService
from .schemas import Goal, SessionOut


def propose_for_goal(calendar: CalendarService, goal: Goal) -> tuple[str, list[SessionOut]]:
    sessions = calendar.propose_goal_sessions(goal)
    if not sessions:
        return (
            "I checked your calendar and couldn't find free slots that fit right now. "
            "We can adjust the cadence or window, then try again — nothing was booked.",
            [],
        )
    lines = [f"• {s.start_at[:16].replace('T', ' ')} — {s.title}" for s in sessions[:8]]
    more = f"\n(+{len(sessions) - 8} more)" if len(sessions) > 8 else ""
    reply = (
        f"Here are {len(sessions)} proposed session(s) based on real availability. "
        "Nothing is booked until you approve.\n"
        + "\n".join(lines)
        + more
    )
    if goal.status in {"gathering", "ready_to_plan"}:
        goal.status = "planned"
    return reply, sessions
