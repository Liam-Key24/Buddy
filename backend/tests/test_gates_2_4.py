"""Gates 2–4: calendar proposals, tracking, sparks, mixed input."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

import pytest

from app.goal_conversation import GoalConversationService


@pytest.fixture()
def service(tmp_path: Path):
    db = tmp_path / "test.db"
    svc = GoalConversationService(db_path=db)
    yield svc
    svc.close()


def _intake_ready(service: GoalConversationService):
    first = service.handle_message("I want to climb V6 by the end of November.")
    second = service.handle_message("V4, twice a week.", conversation_id=first.conversation_id)
    assert second.goal is not None
    assert second.goal.status == "ready_to_plan"
    return second


def test_propose_dated_sessions_from_availability(service: GoalConversationService):
    ready = _intake_ready(service)
    planned = service.handle_message("Yes, look at the calendar.", conversation_id=ready.conversation_id)
    assert planned.proposed_sessions
    assert all(s.status == "proposed" for s in planned.proposed_sessions)
    assert all(s.goal_id == ready.goal.id for s in planned.proposed_sessions)
    # Sessions are dated in the future
    for s in planned.proposed_sessions:
        assert datetime.fromisoformat(s.start_at) > datetime.now()


def test_approve_books_once_and_links_goal(service: GoalConversationService):
    ready = _intake_ready(service)
    planned = service.handle_message("Yes, schedule it.", conversation_id=ready.conversation_id)
    batch = planned.proposed_sessions[0].proposal_batch_id
    assert batch
    approved = service.handle_message("Approve", conversation_id=ready.conversation_id)
    assert approved.booked_sessions
    assert all(s.status == "scheduled" for s in approved.booked_sessions)
    assert all(s.goal_id == ready.goal.id for s in approved.booked_sessions)
    # Duplicate prevention: approving again does not double-book
    again = service.handle_message("Approve", conversation_id=ready.conversation_id)
    assert again.booked_sessions == []


def test_reject_proposal(service: GoalConversationService):
    ready = _intake_ready(service)
    planned = service.handle_message("Propose sessions.", conversation_id=ready.conversation_id)
    assert planned.proposed_sessions
    rejected = service.handle_message("Reject those", conversation_id=ready.conversation_id)
    assert "Rejected" in rejected.reply
    open_batch = service.calendar.open_proposal_batch(ready.goal.id)
    assert open_batch is None


def test_track_completed_and_missed(service: GoalConversationService):
    ready = _intake_ready(service)
    service.handle_message("Yes, look at the calendar.", conversation_id=ready.conversation_id)
    booked = service.handle_message("Approve", conversation_id=ready.conversation_id)
    assert len(booked.booked_sessions) >= 2
    done = service.handle_message(
        "I completed today's climbing session.",
        conversation_id=ready.conversation_id,
    )
    assert "completed" in done.reply.lower()
    missed = service.handle_message(
        "I missed a session.",
        conversation_id=ready.conversation_id,
    )
    assert "missed" in missed.reply.lower()
    progress = service.calendar.progress_for_goal(ready.goal.id)
    assert progress["completed"] >= 1
    assert progress["missed"] >= 1


def test_sparks_capture_without_commitment(service: GoalConversationService):
    res = service.handle_message("Spark: try pottery one weekend")
    assert res.sparks
    assert res.sparks[0].status == "open"
    assert "not a commitment" in res.reply.lower()
    opens = service.sparks.list_open()
    assert len(opens) == 1


def test_mixed_day_dump_preserves_unresolved(service: GoalConversationService):
    res = service.handle_message(
        "I want to climb V6 by the end of November; also spark: visit Lisbon; and buy milk"
    )
    assert res.goal is not None
    assert res.sparks
    assert any("milk" in u.lower() for u in res.unresolved) or "unresolved" in res.reply.lower()


def test_multiple_goals(service: GoalConversationService):
    climb = service.handle_message("I want to climb V6 by the end of November.")
    service.handle_message("V4, twice a week.", conversation_id=climb.conversation_id)
    reading = service.handle_message(
        "Also I want to read 12 books by the end of December.",
        conversation_id=climb.conversation_id,
    )
    assert reading.goal is not None
    assert reading.goal.id != climb.goal.id
    assert reading.goal.domain == "reading"
