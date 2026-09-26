"""Login, session cookie, and /me /logout."""

from __future__ import annotations

from pathlib import Path

from fastapi.testclient import TestClient

from app import main as mainmod
from app.control_plane import ControlPlane


def _client(tmp_path: Path) -> tuple[TestClient, ControlPlane]:
    plane = ControlPlane(db_path=tmp_path / "auth.db", ai=None)
    mainmod.plane.close()
    mainmod.plane = plane
    return TestClient(mainmod.app), plane


def test_login_me_logout(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        denied = client.get("/api/auth/me")
        assert denied.status_code == 401

        bad = client.post("/api/auth/login", json={"username": "liam", "password": "nope"})
        assert bad.status_code == 401

        ok = client.post("/api/auth/login", json={"username": "liam", "password": "test-pass-ok"})
        assert ok.status_code == 200
        assert ok.json()["username"] == "liam"
        assert client.cookies.get("buddy_session")

        me = client.get("/api/auth/me")
        assert me.status_code == 200
        assert me.json()["username"] == "liam"

        out = client.post("/api/auth/logout")
        assert out.status_code == 200
        assert client.get("/api/auth/me").status_code == 401
    finally:
        plane.close()


def test_unauthenticated_api_is_gated(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        assert client.get("/api/health").status_code == 200
        assert client.get("/api/goals").status_code == 401
        assert client.get("/api/conversations").status_code == 401
        assert client.post("/api/sparks", json={"content": "x"}).status_code == 401
    finally:
        plane.close()
