"""Local conversation management uses zero Cloud AI requests."""

from __future__ import annotations

from pathlib import Path

from app.config import DEFAULT_CORS_ORIGINS, Settings
from app.control_plane import ControlPlane


def _plane(tmp_path: Path) -> ControlPlane:
    settings = Settings(
        db_path=tmp_path / "c.db",
        host="127.0.0.1",
        port=8787,
        groq_api_key=None,
        groq_model="openai/gpt-oss-120b",
        groq_base_url="https://api.groq.com/openai/v1",
        ai_enabled=True,
        request_timeout_s=5,
        max_output_tokens=256,
        cors_origins=DEFAULT_CORS_ORIGINS,
    )
    return ControlPlane(db_path=tmp_path / "c.db", settings=settings, ai=None)


def test_new_rename_delete_restore_zero_ai(tmp_path: Path):
    plane = _plane(tmp_path)
    try:
        before = plane.conn.execute("SELECT COUNT(*) AS c FROM ai_requests").fetchone()["c"]
        created = plane.create_conversation()
        assert created["title"] == "New chat"
        renamed = plane.rename_conversation(created["id"], "Climb plan")
        assert renamed["title"] == "Climb plan"
        deleted = plane.delete_conversation(created["id"])
        assert deleted["deleted_at"]
        assert plane.list_conversations() == []
        restored = plane.restore_conversation(created["id"])
        assert restored["deleted_at"] is None
        assert len(plane.list_conversations()) == 1
        after = plane.conn.execute("SELECT COUNT(*) AS c FROM ai_requests").fetchone()["c"]
        assert before == after == 0
    finally:
        plane.close()


def test_undo_booking_without_ai(tmp_path: Path):
    from tests.fake_ai import FakeGroq

    ai = FakeGroq()
    plane = ControlPlane(db_path=tmp_path / "u.db", ai=ai)
    try:
        ready = plane.handle_message("I want to climb V6 by the end of November.")
        plane.handle_message("V4, twice a week.", conversation_id=ready.conversation_id)
        planned = plane.handle_message(
            "Yes, look at the calendar.", conversation_id=ready.conversation_id
        )
        assert planned.proposed_sessions
        batch = planned.proposed_sessions[0].proposal_batch_id
        calls_before = ai.calls
        approved = plane.decide_proposal(batch, "approve", conversation_id=ready.conversation_id)
        assert approved["booked"]
        assert ai.calls == calls_before  # no AI
        undone = plane.decide_proposal(batch, "undo", conversation_id=ready.conversation_id)
        assert undone["undone"]
        assert all(s.status == "proposed" for s in undone["undone"])
        assert ai.calls == calls_before
    finally:
        plane.close()
