"""Gate 1 acceptance: goal conversation through the second answer."""

from __future__ import annotations

from pathlib import Path

import pytest

from app.goal_conversation import GoalConversationService


@pytest.fixture()
def service(tmp_path: Path):
    db = tmp_path / "test.db"
    svc = GoalConversationService(db_path=db)
    yield svc
    svc.close()


def test_v6_conversation_keeps_one_goal_through_second_answer(service: GoalConversationService):
    first = service.handle_message("I want to climb V6 by the end of November.")
    assert first.goal is not None
    goal_id = first.goal.id
    assert first.goal.target == "V6"
    assert first.goal.deadline is not None
    assert "November" in first.goal.title or "november" in first.goal.title.lower()
    assert first.goal.title.lower() != "captured climb v6"
    assert "Captured" not in first.goal.title
    assert first.pending_question
    q = first.pending_question.lower()
    assert "grade" in q or "consistently" in q or "starting" in q

    second = service.handle_message("V4, twice a week.", conversation_id=first.conversation_id)
    assert second.goal is not None
    assert second.goal.id == goal_id
    assert second.goal.target == "V6"
    assert second.goal.baseline == "V4"
    assert second.goal.frequency == "twice a week"
    assert second.goal.status == "ready_to_plan"
    assert "same goal" in second.reply.lower() or second.goal.id == goal_id


def test_reading_goal_asks_relevant_followups(service: GoalConversationService):
    first = service.handle_message("I want to read 12 books by the end of December.")
    assert first.goal is not None
    assert first.goal.domain == "reading"
    assert first.goal.target == "12 books"
    assert first.pending_question

    second = service.handle_message(
        "About one book a month right now, maybe twice a week if I plan it.",
        conversation_id=first.conversation_id,
    )
    assert second.goal is not None
    assert second.goal.id == first.goal.id
    assert second.goal.frequency == "twice a week"


def test_savings_goal_stable_identity(service: GoalConversationService):
    first = service.handle_message("I'd like to save £2000 by the end of March.")
    assert first.goal is not None
    assert first.goal.domain == "savings"
    assert first.goal.target is not None
    assert "2000" in first.goal.target or "£2000" in first.goal.target

    second = service.handle_message(
        "I've got £400 so far and can put money aside once a week.",
        conversation_id=first.conversation_id,
    )
    assert second.goal is not None
    assert second.goal.id == first.goal.id
    assert second.goal.frequency == "once a week"


def test_correction_keeps_same_goal_id(service: GoalConversationService):
    first = service.handle_message("I want to climb V6 by the end of November.")
    goal_id = first.goal.id
    service.handle_message("V4, twice a week.", conversation_id=first.conversation_id)
    corrected = service.handle_message(
        "Actually make the target V7 instead.",
        conversation_id=first.conversation_id,
    )
    assert corrected.goal is not None
    assert corrected.goal.id == goal_id
    assert corrected.goal.target == "V7"


def test_restart_recovers_active_goal(service: GoalConversationService):
    first = service.handle_message("I want to climb V6 by the end of November.")
    cid = first.conversation_id
    goal_id = first.goal.id
    # Simulate restart with same conversation id
    resumed = service.handle_message("V4, twice a week.", conversation_id=cid)
    assert resumed.goal is not None
    assert resumed.goal.id == goal_id
    loaded = service.get_goal(goal_id)
    assert loaded is not None
    assert loaded.baseline == "V4"
