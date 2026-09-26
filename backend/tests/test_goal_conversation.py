"""Goal conversation journeys via control plane + mocked Cloud AI."""

from __future__ import annotations

import re
from pathlib import Path

import pytest

from app.control_plane import ControlPlane
from tests.fake_ai import FakeGroq


@pytest.fixture()
def plane(tmp_path: Path):
    ai = FakeGroq()
    svc = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    yield svc, ai
    svc.close()


def test_v6_conversation_keeps_one_goal_through_second_answer(plane):
    service, ai = plane
    first = service.handle_message("I want to climb V6 by the end of November.")
    assert first.goal is not None
    goal_id = first.goal.id
    assert first.goal.target == "V6"
    assert first.goal.deadline is not None
    assert "November" in first.goal.title or "november" in first.goal.title.lower()
    assert first.pending_question
    q = first.pending_question.lower()
    assert "grade" in q or "consistently" in q or "starting" in q
    assert ai.calls == 1

    second = service.handle_message("V4, twice a week.", conversation_id=first.conversation_id)
    assert second.goal is not None
    assert second.goal.id == goal_id
    assert second.goal.target == "V6"
    assert second.goal.baseline == "V4"
    assert second.goal.frequency == "twice a week"
    assert second.goal.status == "ready_to_plan"
    assert ai.calls == 2


def test_reading_goal_asks_relevant_followups(plane):
    service, _ = plane
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


def test_savings_goal_stable_identity(plane):
    service, _ = plane
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


def test_savings_capacity_not_frequency_wall(plane):
    service, _ = plane
    first = service.handle_message(
        "I want to save £5000 by 1 May 2027. I've got about £1200 so far. "
        "I'm paid on the last day of the month."
    )
    cid = first.conversation_id
    mid = service.handle_message(
        "After expenses I usually have £800-900 left, and I still want spending money.",
        conversation_id=cid,
    )
    assert "how often can you set money aside" not in mid.reply.lower()

    ask = service.handle_message("How much can I save?", conversation_id=cid)
    assert re.search(r"£?\d+", ask.reply)
    assert ask.goal is not None
    assert ask.goal.status == "ready_to_plan"

    planned = service.handle_message(
        "Yes, propose payday sessions on the calendar.",
        conversation_id=cid,
    )
    assert planned.proposed_sessions
    approved = service.handle_message("Approve", conversation_id=cid)
    assert approved.booked_sessions


def test_correction_keeps_same_goal_id(plane):
    service, _ = plane
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


def test_restart_recovers_active_goal(plane):
    service, _ = plane
    first = service.handle_message("I want to climb V6 by the end of November.")
    cid = first.conversation_id
    goal_id = first.goal.id
    resumed = service.handle_message("V4, twice a week.", conversation_id=cid)
    assert resumed.goal is not None
    assert resumed.goal.id == goal_id
    loaded = service.get_goal(goal_id)
    assert loaded is not None
    assert loaded.baseline == "V4"


def test_new_product_goal_keeps_climbing_open(plane):
    service, _ = plane
    climb = service.handle_message("I want to climb V6 by the end of November.")
    mevero = service.handle_message(
        "I want to make a product called mevero i have made 80% of it but need to make the rest by end of december",
        conversation_id=climb.conversation_id,
    )
    assert mevero.goal is not None
    assert mevero.goal.id != climb.goal.id
    assert "mevero" in mevero.goal.title.lower()
    assert "climb v6" not in mevero.reply.lower()
    old = service.get_goal(climb.goal.id)
    assert old is not None
    assert old.status != "paused"
    open_titles = {g.title for g in service.goals.list_open()}
    assert any("climb" in t.lower() for t in open_titles)
    assert any("mevero" in t.lower() for t in open_titles)

    ready = service.handle_message("Twice a week.", conversation_id=mevero.conversation_id)
    assert ready.goal is not None
    assert ready.goal.status == "ready_to_plan"
    planned = service.handle_message("Propose sessions on the calendar.", conversation_id=mevero.conversation_id)
    assert planned.proposed_sessions
    approved = service.handle_message("Approve", conversation_id=mevero.conversation_id)
    assert approved.booked_sessions


def test_one_message_one_ai_call(plane):
    service, ai = plane
    service.handle_message("I want to climb V6 by the end of November.")
    assert ai.calls == 1
