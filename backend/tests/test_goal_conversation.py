"""Gate 1 acceptance: goal conversation through the second answer."""

from __future__ import annotations

import re
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


def test_savings_capacity_not_frequency_wall(service: GoalConversationService):
    """Ambiguous savings: plan with capacity, never stonewall on 'how often'."""
    first = service.handle_message(
        "I want to save £5000 by 1 May 2027. I've got about £1200 so far. "
        "I'm paid on the last day of the month."
    )
    assert first.goal is not None
    cid = first.conversation_id
    assert first.goal.domain == "savings"

    mid = service.handle_message(
        "After expenses I usually have £800-900 left, and I still want spending money.",
        conversation_id=cid,
    )
    assert mid.goal is not None
    assert mid.goal.id == first.goal.id
    # Must not hit the old frequency wall.
    assert "how often can you set money aside" not in mid.reply.lower()
    assert "how often can you set money aside" not in (mid.pending_question or "").lower()

    ask = service.handle_message("How much can I save?", conversation_id=cid)
    assert ask.goal is not None
    assert ask.goal.id == first.goal.id
    assert "how often can you set money aside" not in ask.reply.lower()
    assert re.search(r"£?\d+", ask.reply)  # gives a number estimate
    assert (
        "approve" in ask.reply.lower()
        or "calendar" in ask.reply.lower()
        or "payday" in ask.reply.lower()
    )
    assert ask.goal.status == "ready_to_plan"

    planned = service.handle_message(
        "Yes, propose payday sessions on the calendar.",
        conversation_id=cid,
    )
    assert planned.proposed_sessions
    approved = service.handle_message("Approve", conversation_id=cid)
    assert approved.booked_sessions
    assert all(s.goal_id == first.goal.id for s in approved.booked_sessions)


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


def test_new_product_goal_switches_off_climbing(service: GoalConversationService):
    """Climbing must not stick when the user clearly starts a product goal."""
    climb = service.handle_message("I want to climb V6 by the end of November.")
    ready = service.handle_message("V4, twice a week.", conversation_id=climb.conversation_id)
    assert ready.goal is not None
    assert ready.goal.status == "ready_to_plan"
    climb_id = ready.goal.id
    cid = ready.conversation_id

    mevero = service.handle_message(
        "I want to make a product called mevero i have made 80% of it "
        "but need to make the rest by end of december",
        conversation_id=cid,
    )
    assert mevero.goal is not None
    assert mevero.goal.id != climb_id
    assert mevero.goal.domain == "product"
    assert mevero.goal.target and "mevero" in mevero.goal.target.lower()
    assert mevero.goal.deadline and mevero.goal.deadline.endswith("-12")
    reply = mevero.reply.lower()
    assert "climb v6" not in reply
    assert "leftover money" not in reply
    assert "mevero" in reply or "product" in reply or "december" in reply
    paused = service.get_goal(climb_id)
    assert paused is not None
    assert paused.status == "paused"


def test_mevero_path_propose_and_approve(service: GoalConversationService):
    first = service.handle_message(
        "I want to make a product called mevero i have made 80% of it "
        "but need to make the rest by end of december"
    )
    assert first.goal is not None
    assert first.goal.domain == "product"
    cid = first.conversation_id
    goal_id = first.goal.id

    cadence = service.handle_message(
        "I can commit to three focused sessions a week.",
        conversation_id=cid,
    )
    assert cadence.goal is not None
    assert cadence.goal.id == goal_id
    assert cadence.goal.frequency is not None
    assert cadence.goal.status in {"ready_to_plan", "planned", "gathering"}

    if cadence.goal.status == "gathering":
        cadence = service.handle_message(
            "Three times a week works — let's plan sessions.",
            conversation_id=cid,
        )

    planned = service.handle_message(
        "Yes, propose sessions on the calendar.",
        conversation_id=cid,
    )
    assert planned.goal is not None
    assert planned.goal.id == goal_id
    assert planned.proposed_sessions
    assert "mevero" in planned.reply.lower() or "finish mevero" in (planned.goal.title or "").lower()

    approved = service.handle_message("Approve", conversation_id=cid)
    assert approved.booked_sessions
    assert all(s.goal_id == goal_id for s in approved.booked_sessions)
