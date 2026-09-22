"""Deadline parsing should not strand proposals in the past."""

from __future__ import annotations

from datetime import date, datetime, timedelta
from pathlib import Path

from app.calendar import CalendarService, _parse_deadline_end
from app.control_plane import ControlPlane
from app.goals import GoalStore


def test_parse_deadline_rolls_past_year_forward():
    today = datetime.now().date()
    end = _parse_deadline_end("2023-11")
    assert end >= today
    assert end.month == 11
    assert end.day == 30


def test_parse_deadline_rolls_past_full_date_forward():
    today = datetime.now().date()
    end = _parse_deadline_end("2020-11-30")
    assert end >= today
    assert (end.month, end.day) == (11, 30)


def test_weekly_plan_proposes_when_deadline_year_is_past(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "past_deadline.db", ai=None)
    try:
        cid = plane._ensure_conversation(None)
        stores = GoalStore(plane.conn)
        goal = stores.create(
            cid,
            title="Climb V6",
            domain="climbing",
            target="V6",
            deadline="2023-11-30",
            frequency="3 per week",
            status="ready_to_plan",
            facts={
                "weekly_plan": {
                    "pattern_summary": "Mon/Tue/Thu climb 17:30–19:00",
                    "prefer_after_hour": 17,
                    "window_end_hour": 21,
                    "slots": [
                        {
                            "weekday": 0,
                            "title": "Climb · Mon",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                        {
                            "weekday": 1,
                            "title": "Climb · Tue",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                        {
                            "weekday": 3,
                            "title": "Climb · Thu",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                    ],
                }
            },
        )
        sessions = plane.calendar.propose_goal_sessions(goal)
        assert len(sessions) > 0
        assert all(datetime.fromisoformat(s.start_at).date() >= datetime.now().date() for s in sessions)
    finally:
        plane.conn.close()
