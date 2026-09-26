"""Personal data stays on one account."""

from __future__ import annotations

from pathlib import Path

from fastapi.testclient import TestClient

from app import main as mainmod
from app.control_plane import ControlPlane


def _client(tmp_path: Path) -> tuple[TestClient, ControlPlane]:
    plane = ControlPlane(db_path=tmp_path / "iso.db", ai=None)
    mainmod.plane.close()
    mainmod.plane = plane
    return TestClient(mainmod.app), plane


def _login(client: TestClient, username: str) -> None:
    client.cookies.clear()
    res = client.post("/api/auth/login", json={"username": username, "password": "test-pass-ok"})
    assert res.status_code == 200


def test_owner_cannot_read_or_write_other_users_rows(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        _login(client, "liam")
        spark = client.post("/api/sparks", json={"content": "liam-only idea"}).json()
        goal = plane.goals.create(
            plane.create_conversation()["id"],
            title="Liam climb",
            status="active",
        )
        session = plane.calendar.create_manual_session(
            title="Liam session",
            start_at="2026-10-01T17:00:00",
            end_at="2026-10-01T18:00:00",
        )
        conv = client.post("/api/conversations").json()

        _login(client, "partner")
        sparks = client.get("/api/sparks").json()
        assert all(row["id"] != spark["id"] for row in sparks)
        assert all("liam-only" not in row["content"] for row in sparks)

        assert client.get(f"/api/goals/{goal.id}").status_code == 404
        goals = client.get("/api/goals").json()
        assert all(row["id"] != goal.id for row in goals)

        sessions = client.get("/api/calendar/sessions").json()
        assert all(row["id"] != session.id for row in sessions)
        assert client.delete(f"/api/calendar/sessions/{session.id}").status_code == 404

        chats = client.get("/api/conversations").json()
        assert all(row["id"] != conv["id"] for row in chats)
        assert client.get(f"/api/conversations/{conv['id']}/messages").status_code == 404

        mine = client.post("/api/sparks", json={"content": "partner idea"}).json()
        _login(client, "liam")
        liam_sparks = client.get("/api/sparks").json()
        assert any(row["id"] == spark["id"] for row in liam_sparks)
        assert all(row["id"] != mine["id"] for row in liam_sparks)
    finally:
        plane.close()


def test_revert_and_cancel_reject_other_owner(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        users = {r["username"]: r["id"] for r in plane.conn.execute("SELECT id, username FROM users")}
        liam = users["liam"]
        partner = users["partner"]
        conv = plane.create_conversation(owner_user_id=liam)
        result = plane.revert_to(conv["id"], "missing-message", owner_user_id=partner)
        assert result.get("ok") is False

        plane._active_requests["steal-me"] = {
            "cancelled": False,
            "http_client": None,
            "owns_http_client": False,
            "owner_user_id": liam,
        }
        cancelled = plane.cancel_request("steal-me", owner_user_id=partner)
        assert cancelled["found"] is False
        assert plane._active_requests["steal-me"]["cancelled"] is False
    finally:
        plane.close()


def test_resolve_owner_fails_closed_without_fallback(tmp_path: Path, monkeypatch):
    from app.owners import resolve_owner

    monkeypatch.delenv("BUDDY_OWNER_FALLBACK", raising=False)
    plane = ControlPlane(db_path=tmp_path / "closed.db", ai=None)
    try:
        try:
            resolve_owner(plane.conn)
        except RuntimeError as exc:
            assert "owner_user_id required" in str(exc)
        else:
            raise AssertionError("expected owner_user_id required")
    finally:
        plane.close()
