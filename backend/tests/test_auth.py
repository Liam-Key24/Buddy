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


def test_login_is_case_insensitive(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        ok = client.post("/api/auth/login", json={"username": "LIAM", "password": "test-pass-ok"})
        assert ok.status_code == 200
        assert ok.json()["username"] == "liam"
    finally:
        plane.close()


def test_login_rate_limit_locks_after_failures(tmp_path: Path):
    from app import auth as authmod

    client, plane = _client(tmp_path)
    try:
        authmod._login_failures.clear()
        for _ in range(5):
            bad = client.post("/api/auth/login", json={"username": "liam", "password": "nope"})
            assert bad.status_code == 401
        locked = client.post("/api/auth/login", json={"username": "liam", "password": "nope"})
        assert locked.status_code == 429
        still = client.post("/api/auth/login", json={"username": "liam", "password": "test-pass-ok"})
        assert still.status_code == 429
    finally:
        authmod._login_failures.clear()
        plane.close()


def test_set_password_revokes_sessions(tmp_path: Path):
    from app.auth import set_password

    client, plane = _client(tmp_path)
    try:
        users = {r["username"]: r["id"] for r in plane.conn.execute("SELECT id, username FROM users")}
        client.post("/api/auth/login", json={"username": "liam", "password": "test-pass-ok"})
        assert client.get("/api/auth/me").status_code == 200
        set_password(plane.conn, users["liam"], "new-pass-ok")
        assert client.get("/api/auth/me").status_code == 401
        again = client.post("/api/auth/login", json={"username": "liam", "password": "new-pass-ok"})
        assert again.status_code == 200
    finally:
        plane.close()


def test_security_headers_present(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        res = client.get("/api/health")
        assert res.headers.get("x-frame-options") == "DENY"
        assert res.headers.get("x-content-type-options") == "nosniff"
        assert "frame-ancestors 'none'" in res.headers.get("content-security-policy", "")
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
