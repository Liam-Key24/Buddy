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
        # First week has 4 distinct session kinds
        sample = summary["sample"]
        assert len(sample) == 4
        sample_titles = [s.title for s in sample]
        assert len(set(sample_titles)) == 4
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
