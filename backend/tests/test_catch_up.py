"""Catch-up proposals for missed sessions before the goal deadline."""

from __future__ import annotations

from datetime import datetime, timedelta
from pathlib import Path

from app.control_plane import ControlPlane
from app.goals import GoalStore
from app.planning import propose_for_goal


def test_propose_stacks_catch_up_for_missed(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "catchup.db", ai=None)
    try:
        cid = plane._ensure_conversation(None)
        stores = GoalStore(plane.conn)
        deadline = (datetime.now().date() + timedelta(days=45)).isoformat()
        goal = stores.create(
            cid,
            title="Climb V6",
            domain="climbing",
            target="V6",
            deadline=deadline,
            frequency="2 per week",
            status="ready_to_plan",
            facts={
                "weekly_plan": {
                    "pattern_summary": "Mon/Thu climb 17:30–19:00",
                    "avoid_weekdays": [2],
                    "prefer_after_hour": 17,
                    "window_end_hour": 21,
                    "slots": [
                        {
                            "weekday": 0,
                            "title": "Climb · Technique",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                        {
                            "weekday": 3,
                            "title": "Climb · Endurance",
                            "start_hour": 17,
                            "start_minute": 30,
                            "duration_minutes": 90,
                        },
                    ],
                }
            },
        )

        # Seed two past missed sessions for this goal.
        now = datetime.now()
        for i in range(2):
            start = now - timedelta(days=7 + i, hours=2)
            end = start + timedelta(minutes=90)
            plane.conn.execute(
                """
                INSERT INTO sessions (
                    id, goal_id, title, start_at, end_at, kind, status,
                    proposal_batch_id, notes, created_at, updated_at, category_id
                ) VALUES (?, ?, ?, ?, ?, 'flexible', 'missed', NULL, NULL, ?, ?, NULL)
                """,
                (
                    f"missed-{i}",
                    goal.id,
                    "Climb · Technique",
                    start.isoformat(),
                    end.isoformat(),
                    now.isoformat(),
                    now.isoformat(),
                ),
            )
        plane.conn.commit()

        text, sessions, summary = propose_for_goal(plane.calendar, goal)
        catch_ups = [s for s in sessions if (s.notes or "") == "catch-up"]
        assert len(catch_ups) == 2
        assert all("Catch-up" in s.title for s in catch_ups)
        assert all(datetime.fromisoformat(s.start_at).date() <= datetime.fromisoformat(deadline).date() for s in catch_ups)
        assert summary.get("why", {}).get("catch_up_sessions") == 2
        assert "catch-up" in text.lower()
        # Still proposals, not booked
        assert all(s.status == "proposed" for s in sessions)
    finally:
        plane.close()
