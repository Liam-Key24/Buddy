"""Gates 2–4: calendar proposals, tracking, sparks, mixed input."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

import pytest

from app.control_plane import ControlPlane
from tests.fake_ai import FakeGroq


@pytest.fixture()
def plane(tmp_path: Path):
    ai = FakeGroq()
    svc = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    yield svc
    svc.close()


def _intake_ready(service: ControlPlane):
    first = service.handle_message("I want to climb V6 by the end of November.")
    second = service.handle_message("V4, twice a week.", conversation_id=first.conversation_id)
    assert second.goal is not None
    assert second.goal.status == "ready_to_plan"
    return second


def test_propose_dated_sessions_from_availability(plane: ControlPlane):
    ready = _intake_ready(plane)
    planned = plane.handle_message("Yes, look at the calendar.", conversation_id=ready.conversation_id)
    assert planned.proposed_sessions
    assert all(s.status == "proposed" for s in planned.proposed_sessions)
    assert all(s.goal_id == ready.goal.id for s in planned.proposed_sessions)
    for s in planned.proposed_sessions:
        assert datetime.fromisoformat(s.start_at) > datetime.now()


def test_approve_books_once_and_links_goal(plane: ControlPlane):
    ready = _intake_ready(plane)
    planned = plane.handle_message("Yes, schedule it.", conversation_id=ready.conversation_id)
    assert planned.proposed_sessions
    approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    assert approved.booked_sessions
    assert all(s.status == "scheduled" for s in approved.booked_sessions)
    assert all(s.goal_id == ready.goal.id for s in approved.booked_sessions)
    again = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    assert again.booked_sessions == []


def test_reject_proposal(plane: ControlPlane):
    ready = _intake_ready(plane)
    planned = plane.handle_message("Propose sessions.", conversation_id=ready.conversation_id)
    assert planned.proposed_sessions
    rejected = plane.handle_message("Reject those", conversation_id=ready.conversation_id)
    assert "Rejected" in rejected.reply
    open_batch = plane.calendar.open_proposal_batch(ready.goal.id)
    assert open_batch is None


def test_track_completed_and_missed(plane: ControlPlane):
    ready = _intake_ready(plane)
    plane.handle_message("Yes, look at the calendar.", conversation_id=ready.conversation_id)
    booked = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    assert len(booked.booked_sessions) >= 2
    # Use explicit API outcome on first booked session (deterministic, no AI)
    first_id = booked.booked_sessions[0].id
    plane.mark_outcome(first_id, "completed")
    second_id = booked.booked_sessions[1].id
    plane.mark_outcome(second_id, "missed")
    progress = plane.calendar.progress_for_goal(ready.goal.id)
    assert progress["completed"] >= 1
    assert progress["missed"] >= 1


def test_spark_capture(plane: ControlPlane):
    res = plane.handle_message("spark: try a standing desk")
    assert res.sparks
    assert "standing desk" in res.sparks[0].content.lower()


def test_invalid_buddy_turn_does_not_mutate(tmp_path: Path):
    class BadAI:
        calls = 0

        def complete_json(self, system, user, *, allow_retry=True):
            self.calls += 1
            return {"nope": True}

        def close(self):
            pass

    ai = BadAI()
    svc = ControlPlane(db_path=tmp_path / "bad.db", ai=ai)
    try:
        before = svc.conn.execute("SELECT COUNT(*) AS c FROM goals").fetchone()["c"]
        res = svc.handle_message("I want to climb V6")
        after = svc.conn.execute("SELECT COUNT(*) AS c FROM goals").fetchone()["c"]
        assert before == after
        assert res.ai_available is False
    finally:
        svc.close()
