"""Additive SQLite migrations. Never drop existing tables."""

from __future__ import annotations

import sqlite3
from datetime import datetime, timezone


SCHEMA_VERSION = 1


def _ensure_meta(conn: sqlite3.Connection) -> None:
    conn.execute(
        """
        CREATE TABLE IF NOT EXISTS buddy_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        )
        """
    )
    conn.commit()


def get_schema_version(conn: sqlite3.Connection) -> int:
    _ensure_meta(conn)
    row = conn.execute("SELECT value FROM buddy_meta WHERE key = 'schema_version'").fetchone()
    if not row:
        return 0
    try:
        return int(row["value"] if isinstance(row, sqlite3.Row) else row[0])
    except ValueError:
        return 0


def set_schema_version(conn: sqlite3.Connection, version: int) -> None:
    _ensure_meta(conn)
    now = datetime.now(timezone.utc).isoformat()
    conn.execute(
        """
        INSERT INTO buddy_meta (key, value) VALUES ('schema_version', ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        """,
        (str(version),),
    )
    conn.execute(
        """
        INSERT INTO buddy_meta (key, value) VALUES ('schema_migrated_at', ?)
        ON CONFLICT(key) DO UPDATE SET value = excluded.value
        """,
        (now,),
    )
    conn.commit()


def run_migrations(conn: sqlite3.Connection) -> int:
    """Apply additive migrations. Returns resulting schema version."""
    version = get_schema_version(conn)
    # v1: meta table only — base tables already created by init_db / existing installs.
    if version < 1:
        _ensure_meta(conn)
        set_schema_version(conn, 1)
        version = 1
    return version
