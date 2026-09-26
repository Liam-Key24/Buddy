"""Each account has its own daily Cloud AI request allowance."""

from __future__ import annotations

from pathlib import Path

from fastapi.testclient import TestClient

from app import main as mainmod
from app.control_plane import ControlPlane
from tests.fake_ai import FakeGroq


def _users(plane: ControlPlane) -> dict[str, str]:
    rows = plane.conn.execute("SELECT id, username FROM users").fetchall()
    return {row["username"]: row["id"] for row in rows}


def test_usage_limit_is_500_per_owner(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "quota.db", ai=None)
    try:
        users = _users(plane)
        liam = users["liam"]
        partner = users["partner"]
        plane.usage.record(conversation_id=None, model="test", status="ok", owner_user_id=liam)
        liam_summary = plane.usage.today_summary(liam)
        partner_summary = plane.usage.today_summary(partner)
        assert liam_summary["limit"] == 500
        assert liam_summary["used"] == 1
        assert liam_summary["remaining"] == 499
        assert partner_summary["used"] == 0
        assert partner_summary["remaining"] == 500
    finally:
        plane.close()


def test_chat_blocks_when_owner_hits_daily_limit(tmp_path: Path, monkeypatch):
    monkeypatch.setenv("BUDDY_DAILY_REQUEST_LIMIT", "1")
    fake = FakeGroq()
    plane = ControlPlane(db_path=tmp_path / "quota-block.db", ai=fake)
    try:
        users = _users(plane)
        liam = users["liam"]
        partner = users["partner"]
        plane.usage.record(conversation_id=None, model="test", status="ok", owner_user_id=liam)

        blocked = plane.handle_message("I want to climb V6", owner_user_id=liam)
        assert fake.calls == 0
        assert blocked.ai_available is False
        assert "1 Cloud AI requests" in blocked.reply or "1" in blocked.reply
        assert "tomorrow" in blocked.reply.lower()

        other = plane.handle_message("I want to climb V6", owner_user_id=partner)
        assert fake.calls == 1
        assert other.ai_available is True
    finally:
        plane.close()


def test_usage_endpoint_is_owner_scoped(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "quota-api.db", ai=None)
    mainmod.plane.close()
    mainmod.plane = plane
    client = TestClient(mainmod.app)
    try:
        users = _users(plane)
        plane.usage.record(
            conversation_id=None, model="test", status="ok", owner_user_id=users["liam"]
        )
        client.post("/api/auth/login", json={"username": "liam", "password": "test-pass-ok"})
        liam = client.get("/api/ai/usage").json()
        assert liam["used"] == 1
        assert liam["limit"] == 500

        client.cookies.clear()
        client.post("/api/auth/login", json={"username": "partner", "password": "test-pass-ok"})
        partner = client.get("/api/ai/usage").json()
        assert partner["used"] == 0
        assert partner["limit"] == 500
    finally:
        plane.close()
