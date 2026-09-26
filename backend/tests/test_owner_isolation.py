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
