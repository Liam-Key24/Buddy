"""Seed private couple accounts for every backend test."""

from __future__ import annotations

import os
import tempfile
from pathlib import Path

import pytest

# Set before any test module imports app.main (ControlPlane starts at import).
_IMPORT_DB = Path(tempfile.mkdtemp(prefix="buddy-pytest-")) / "import.db"
os.environ.setdefault("BUDDY_USER_1", "liam:test-pass-ok")
os.environ.setdefault("BUDDY_USER_2", "partner:test-pass-ok")
os.environ.setdefault("BUDDY_MIGRATE_OWNER", "liam")
os.environ.setdefault("BUDDY_SESSION_SECRET", "test-session-secret")
os.environ.setdefault("BUDDY_COOKIE_SECURE", "0")
os.environ.setdefault("BUDDY_OWNER_FALLBACK", "1")
os.environ.setdefault("BUDDY_DB_PATH", str(_IMPORT_DB))


@pytest.fixture(autouse=True)
def buddy_auth_env(monkeypatch):
    monkeypatch.setenv("BUDDY_USER_1", "liam:test-pass-ok")
    monkeypatch.setenv("BUDDY_USER_2", "partner:test-pass-ok")
    monkeypatch.setenv("BUDDY_MIGRATE_OWNER", "liam")
    monkeypatch.setenv("BUDDY_SESSION_SECRET", "test-session-secret")
    monkeypatch.setenv("BUDDY_COOKIE_SECURE", "0")
    monkeypatch.setenv("BUDDY_OWNER_FALLBACK", "1")
