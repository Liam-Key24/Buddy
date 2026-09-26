"""Pre-v9 rows are assigned to BUDDY_MIGRATE_OWNER."""

from __future__ import annotations

from pathlib import Path
from unittest.mock import patch

from app.db import get_connection, init_db
from app.migrations import get_schema_version, run_migrations


def test_pre_v9_rows_assigned_to_migrate_owner(tmp_path: Path):
    conn = get_connection(tmp_path / "legacy.db")
    with patch("app.db.run_migrations", lambda c: run_migrations(c, target=8)):
        init_db(conn)
    assert get_schema_version(conn) == 8

    conn.execute(
        "INSERT INTO conversations (id, created_at, updated_at) VALUES ('c-legacy', 't', 't')"
    )
    conn.execute(
        """
        INSERT INTO sparks (id, content, status, created_at, updated_at)
        VALUES ('s-legacy', 'old idea', 'open', 't', 't')
        """
    )
    conn.commit()
    assert not any(
        r[1] == "owner_user_id"
        for r in conn.execute("PRAGMA table_info(conversations)").fetchall()
    )

    version = run_migrations(conn)
    assert version == 9
    owner = conn.execute("SELECT id FROM users WHERE username='liam'").fetchone()
    partner = conn.execute("SELECT id FROM users WHERE username='partner'").fetchone()
    assert owner and partner
    conv = conn.execute(
        "SELECT owner_user_id FROM conversations WHERE id='c-legacy'"
    ).fetchone()
    spark = conn.execute("SELECT owner_user_id FROM sparks WHERE id='s-legacy'").fetchone()
    assert conv["owner_user_id"] == owner["id"]
    assert spark["owner_user_id"] == owner["id"]
    conn.close()


def test_bootstrap_fails_without_env(tmp_path: Path, monkeypatch):
    monkeypatch.delenv("BUDDY_USER_1", raising=False)
    monkeypatch.delenv("BUDDY_USER_2", raising=False)
    monkeypatch.delenv("BUDDY_MIGRATE_OWNER", raising=False)
    from app.control_plane import ControlPlane

    try:
        ControlPlane(db_path=tmp_path / "empty.db", ai=None)
    except RuntimeError as exc:
        assert "bootstrap" in str(exc).lower() or "BUDDY_USER" in str(exc)
    else:
        raise AssertionError("expected startup to fail without bootstrap env")
