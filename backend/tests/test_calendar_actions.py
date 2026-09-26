"""Chat-driven calendar delete, update, move, and outcome actions."""

from __future__ import annotations

from pathlib import Path

import pytest

from app.control_plane import ControlPlane
from app.schemas import CalendarAction
from tests.fake_ai import FakeGroq


@pytest.fixture()
def plane(tmp_path: Path):
    ai = FakeGroq()
    svc = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    yield svc
    svc.close()


def _book_sessions(service: ControlPlane):
    first = service.handle_message("I want to climb V6 by the end of November.")
    second = service.handle_message("V4, twice a week.", conversation_id=first.conversation_id)
    planned = service.handle_message("Yes, look at the calendar.", conversation_id=second.conversation_id)
    booked = service.handle_message("Approve", conversation_id=second.conversation_id)
    assert booked.booked_sessions
    return second, booked


def test_delete_session_by_id(plane: ControlPlane):
    ready, booked = _book_sessions(plane)
    target_id = booked.booked_sessions[0].id
    deleted, updated, errors = plane.calendar.apply_calendar_action(
        CalendarAction(op="delete", session_id=target_id)
    )
    assert deleted == [target_id]
    assert not errors
    assert plane.calendar.get_session(target_id) is None


def test_delete_all_proposed_via_chat(plane: ControlPlane):
    ready = plane.handle_message("I want to climb V6 by the end of November.")
    ready = plane.handle_message("V4, twice a week.", conversation_id=ready.conversation_id)
    planned = plane.handle_message("Yes, look at the calendar.", conversation_id=ready.conversation_id)
    assert planned.proposed_sessions
    proposed_ids = {s.id for s in planned.proposed_sessions}

    result = plane.handle_message("Delete all proposed sessions", conversation_id=ready.conversation_id)
    assert any(op.get("status") == "needs_approval" for op in result.operations)
    remaining_before = plane.calendar.list_sessions(statuses=["proposed"])
    assert any(s.id in proposed_ids for s in remaining_before)
    approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    remaining = plane.calendar.list_sessions(statuses=["proposed"])
    assert not any(s.id in proposed_ids for s in remaining)
    assert approved.deleted_session_ids


def test_delete_single_session_via_chat(plane: ControlPlane):
    ready, booked = _book_sessions(plane)
    target_id = booked.booked_sessions[0].id
    result = plane.handle_message("Delete that session", conversation_id=ready.conversation_id)
    assert target_id not in (result.deleted_session_ids or [])
    assert plane.calendar.get_session(target_id) is not None
    approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    assert target_id in approved.deleted_session_ids
    assert plane.calendar.get_session(target_id) is None


def test_rename_session_via_chat(plane: ControlPlane):
    ready, booked = _book_sessions(plane)
    target = booked.booked_sessions[0]
    result = plane.handle_message("Rename it to power day", conversation_id=ready.conversation_id)
    assert not result.updated_sessions
    approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    assert approved.updated_sessions
    updated = plane.calendar.get_session(target.id)
    assert updated is not None
    assert "Power Day" in updated.title or "power day" in updated.title.lower()


def test_move_session_via_chat(plane: ControlPlane):
    ready, booked = _book_sessions(plane)
    target = booked.booked_sessions[0]
    result = plane.handle_message("Move it to Friday evening", conversation_id=ready.conversation_id)
    assert not result.updated_sessions
    approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
    assert approved.updated_sessions
    updated = plane.calendar.get_session(target.id)
    assert updated is not None
    assert updated.start_at.startswith("2026-12-05")


def test_mark_completed_via_chat(plane: ControlPlane):
    ready, booked = _book_sessions(plane)
    target = booked.booked_sessions[0]
    result = plane.handle_message("I completed that session", conversation_id=ready.conversation_id)
    assert result.updated_sessions
    updated = plane.calendar.get_session(target.id)
    assert updated is not None
    assert updated.status == "completed"
