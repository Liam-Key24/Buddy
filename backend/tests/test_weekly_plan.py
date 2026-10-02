"""Weekly plan proposals: titled slots, pattern summary, horizon to deadline."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

from app.control_plane import ControlPlane
from app.goals import GoalStore
from app.schemas import Goal


def test_weekly_plan_proposes_titled_sessions_through_deadline(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "plan.db", ai=None)
    try:
        # Offline AI path unused — seed goal + plan directly, then propose via calendar.
        cid = plane._ensure_conversation(None)
        stores = GoalStore(plane.conn)
        goal = stores.create(
            cid,
            title="Climb V6",
            domain="climbing",
            target="V6",
            deadline="2026-12",
            baseline="V3",
            frequency="4 per week",
            status="ready_to_plan",
            facts={
                "weekly_plan": {
                    "pattern_summary": (
                        "Mon/Tue/Fri climb 17:30–19:00; Thu strength 17:30–18:30; Wed off"
                    ),
                    "avoid_weekdays": [2],
                    "prefer_after_hour": 17,
                    "window_end_hour": 21,
                    "slots": [
                        {
                            "weekday": 0,
                            "title": "Climb · Technique & movement",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                        {
                            "weekday": 1,
                            "title": "Climb · Endurance",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                        {
                            "weekday": 3,
                            "title": "Climb strength · Hangboard & pull-ups",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 60,
                        },
                        {
                            "weekday": 4,
                            "title": "Climb · Power",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                    ],
                }
            },
        )
        from app.planning import propose_for_goal

        text, sessions, summary = propose_for_goal(plane.calendar, goal)
        assert sessions
        assert summary["total"] == len(sessions)
        assert summary["pattern"]
        assert "Wed" not in (summary["pattern"] or "") or "off" in (summary["pattern"] or "").lower()
        titles = {s.title for s in sessions}
        assert "Climb · Technique & movement" in titles
        assert "Climb strength · Hangboard & pull-ups" in titles
        assert "Climb V6" not in titles or len(titles) > 1
        # Through December, not only 4 weeks of Mon/Tue clones
        last = max(datetime.fromisoformat(s.start_at) for s in sessions)
        assert last.month >= 11 or last.year > datetime.now().year
        # Distinct session kinds across the plan (first ISO week may be partial)
        assert len(titles) == 4
        sample = summary["sample"]
        assert sample
        sample_titles = [s.title for s in sample]
        assert len(set(sample_titles)) == len(sample_titles)
        assert "Weekly pattern" in text or "pattern" in text.lower()
        # No Wednesday sessions
        assert all(datetime.fromisoformat(s.start_at).weekday() != 2 for s in sessions)
    finally:
        plane.close()


def test_frequency_parses_ranges():
    from app.calendar import frequency_to_weekly_count

    assert frequency_to_weekly_count("3-4") == 4
    assert frequency_to_weekly_count("4 per week") == 4
    assert frequency_to_weekly_count("twice a week") == 2
    assert frequency_to_weekly_count("8 pm") == 1
    assert frequency_to_weekly_count("8pm") == 1
    assert frequency_to_weekly_count("at 8pm") == 1
    assert frequency_to_weekly_count("3 per week at 8pm") == 3
    assert frequency_to_weekly_count("once") == 1
    assert frequency_to_weekly_count("single event") == 1
    assert frequency_to_weekly_count("once a week") == 1


def test_null_plan_times_still_propose_sessions(tmp_path: Path):
    """Cloud AI often sends JSON null for hours. That must not raise TypeError."""
    plane = ControlPlane(db_path=tmp_path / "null-times.db", ai=None)
    try:
        cid = plane._ensure_conversation(None)
        stores = GoalStore(plane.conn)
        goal = stores.create(
            cid,
            title="Save £3,000 by 1 April 2027",
            target="£3,000",
            deadline="2027-04-01",
            frequency="monthly",
            status="ready_to_plan",
            facts={
                "weekly_plan": {
                    "pattern_summary": "Transfer savings once a week",
                    "prefer_after_hour": None,
                    "window_end_hour": None,
                    "avoid_weekdays": [None, "Saturday"],
                    "slots": [
                        {
                            "weekday": "Monday",
                            "title": "Transfer savings",
                            "start_hour": None,
                            "start_minute": None,
                            "duration_minutes": None,
                        }
                    ],
                }
            },
        )
        from app.planning import propose_for_goal

        text, sessions, summary = propose_for_goal(plane.calendar, goal)
        assert sessions
        assert summary["total"] == len(sessions)
        assert all(s.title == "Transfer savings" for s in sessions)
        assert "TypeError" not in text
        assert all(datetime.fromisoformat(s.start_at).weekday() == 0 for s in sessions)
    finally:
        plane.close()


def test_one_off_8pm_is_a_single_session(tmp_path: Path):
    from datetime import timedelta

    plane = ControlPlane(db_path=tmp_path / "oneoff.db", ai=None)
    try:
        cid = plane._ensure_conversation(None)
        stores = GoalStore(plane.conn)
        on_date = (datetime.now().date() + timedelta(days=1)).isoformat()
        goal = stores.create(
            cid,
            title="Pick my nose",
            frequency="once",
            status="ready_to_plan",
            facts={
                "weekly_plan": {
                    "repeat": "once",
                    "on_date": on_date,
                    "pattern_summary": "Tue 20:00",
                    "prefer_after_hour": 20,
                    "window_end_hour": 21,
                    "slots": [
                        {
                            "weekday": 1,
                            "title": "Pick my nose",
                            "start_hour": 20,
                            "start_minute": 0,
                            "duration_minutes": 15,
                        }
                    ],
                }
            },
        )
        from app.planning import propose_for_goal

        text, sessions, summary = propose_for_goal(plane.calendar, goal)
        assert len(sessions) == 1
        assert summary["total"] == 1
        start = datetime.fromisoformat(sessions[0].start_at)
        assert start.hour == 20
        assert start.date().isoformat() == on_date
        assert "8 sessions" not in text.lower()
        assert "weekly pattern" not in text.lower()
        assert "one session" in text.lower()
    finally:
        plane.close()


def test_plan_on_propose_payload_is_written_to_the_calendar(tmp_path: Path):
    """Groq puts weekly_plan on propose_sessions, not on the goal. That still has to schedule."""

    class PlanOnPropose:
        def complete_json(self, system: str, user: str, *, allow_retry: bool = True, **kwargs) -> dict:
            del system, user, allow_retry, kwargs
            return {
                "assistant_text": "Monday evening transfers, starting at 17:30.",
                "intents": ["goal_create", "goal_plan_request"],
                "operations": [
                    {
                        "kind": "goal_create",
                        "target_ref": "savings",
                        "payload": {
                            "title": "Save £3,000",
                            "deadline": "2027-04-01",
                            "target": "£3,000",
                            "frequency": "weekly",
                            "status": "gathering",
                            "facts": {},
                        },
                    },
                    {
                        "kind": "propose_sessions",
                        "target_ref": "savings",
                        "payload": {
                            "weekly_plan": {
                                "pattern_summary": "Monday transfer 17:30",
                                "repeat": "weekly",
                                "prefer_after_hour": 17,
                                "window_end_hour": 21,
                                "slots": [
                                    {
                                        "weekday": 0,
                                        "title": "Transfer savings",
                                        "start_hour": 17,
                                        "start_minute": 30,
                                        "duration_minutes": 30,
                                    }
                                ],
                            }
                        },
                    },
                ],
                "goal_updates": [],
                "requested_action": {"type": "propose_sessions"},
                "confidence": 0.9,
            }

        def close(self) -> None:
            return None

    plane = ControlPlane(db_path=tmp_path / "apply-plan.db", ai=PlanOnPropose())
    try:
        result = plane.handle_message(
            "Save £3,000 by 1 April 2027. Transfer £100 every Monday evening after work."
        )
        proposed = result.proposed_sessions or []
        assert proposed, result.reply
        assert all(s.title == "Transfer savings" for s in proposed)
        starts = [datetime.fromisoformat(s.start_at) for s in proposed]
        assert all(start.weekday() == 0 for start in starts)
        assert all(start.hour == 17 and start.minute == 30 for start in starts)
        assert result.goal is not None
        slots = ((result.goal.facts or {}).get("weekly_plan") or {}).get("slots") or []
        assert slots
    finally:
        plane.close()


def test_empty_operation_uses_goal_update_title_and_propose_plan(tmp_path: Path):
    """The model often leaves operations[].payload empty and puts the goal on goal_updates."""

    class SplitFields:
        def complete_json(self, system: str, user: str, *, allow_retry: bool = True, **kwargs) -> dict:
            del system, user, allow_retry, kwargs
            return {
                "assistant_text": "Monday evening transfers, starting at 17:30.",
                "intents": ["goal_create", "goal_plan_request"],
                "operations": [
                    {"kind": "goal_create", "target_ref": "goal_1", "payload": {}},
                    {
                        "kind": "propose_sessions",
                        "target_ref": "goal_1",
                        "payload": {
                            "weekly_plan": {
                                "pattern_summary": "Monday transfer 17:30",
                                "repeat": "weekly",
                                "prefer_after_hour": 17,
                                "window_end_hour": 21,
                                "slots": [
                                    {
                                        "weekday": "Monday",
                                        "title": "Transfer savings",
                                        "start_hour": 17,
                                        "start_minute": 30,
                                        "duration_minutes": 30,
                                    }
                                ],
                            }
                        },
                    },
                ],
                "goal_updates": [
                    {
                        "action": "create",
                        "title": "Save £3,000",
                        "target": "£3,000",
                        "deadline": "2027-04-01",
                        "frequency": "weekly",
                        "status": "gathering",
                        "facts": {},
                    }
                ],
                "requested_action": {"type": "propose_sessions"},
                "confidence": 0.9,
            }

        def close(self) -> None:
            return None

    plane = ControlPlane(db_path=tmp_path / "split-fields.db", ai=SplitFields())
    try:
        result = plane.handle_message(
            "Save £3,000 by 1 April 2027. Transfer £100 every Monday evening after work."
        )
        assert result.goal is not None
        assert result.goal.title == "Save £3,000"
        proposed = result.proposed_sessions or []
        assert proposed, result.reply
        assert all(s.title == "Transfer savings" for s in proposed)
        starts = [datetime.fromisoformat(s.start_at) for s in proposed]
        assert all(start.weekday() == 0 and start.hour == 17 and start.minute == 30 for start in starts)
    finally:
        plane.close()


def test_chat_8pm_single_event_is_one_session(tmp_path: Path):
    from tests.fake_ai import FakeGroq

    plane = ControlPlane(db_path=tmp_path / "chat-oneoff.db", ai=FakeGroq())
    try:
        result = plane.handle_message(
            "goal today to pick my nose at 8 pm today single event"
        )
        proposed = result.proposed_sessions or []
        assert len(proposed) == 1
        start = datetime.fromisoformat(proposed[0].start_at)
        assert start.hour == 20
        assert "weekly pattern" not in (result.reply or "").lower()
        assert "8 sessions" not in (result.reply or "").lower()
    finally:
        plane.close()
