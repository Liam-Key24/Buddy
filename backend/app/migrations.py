"""Additive SQLite migrations. Never drop existing tables."""

from __future__ import annotations

import sqlite3
from datetime import datetime, timezone


SCHEMA_VERSION = 8


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


def _column_exists(conn: sqlite3.Connection, table: str, column: str) -> bool:
    rows = conn.execute(f"PRAGMA table_info({table})").fetchall()
    names = {r["name"] if isinstance(r, sqlite3.Row) else r[1] for r in rows}
    return column in names


def run_migrations(conn: sqlite3.Connection) -> int:
    """Apply additive migrations. Returns resulting schema version."""
    version = get_schema_version(conn)
    if version < 1:
        _ensure_meta(conn)
        set_schema_version(conn, 1)
        version = 1

    if version < 2:
        # Conversation management
        if not _column_exists(conn, "conversations", "title"):
            conn.execute("ALTER TABLE conversations ADD COLUMN title TEXT")
        if not _column_exists(conn, "conversations", "deleted_at"):
            conn.execute("ALTER TABLE conversations ADD COLUMN deleted_at TEXT")
        if not _column_exists(conn, "conversations", "draft_json"):
            conn.execute("ALTER TABLE conversations ADD COLUMN draft_json TEXT NOT NULL DEFAULT '{}'")

        # Message revisions (immutable history)
        if not _column_exists(conn, "messages", "revision_group"):
            conn.execute("ALTER TABLE messages ADD COLUMN revision_group TEXT")
        if not _column_exists(conn, "messages", "revision_of"):
            conn.execute("ALTER TABLE messages ADD COLUMN revision_of TEXT")
        if not _column_exists(conn, "messages", "superseded"):
            conn.execute("ALTER TABLE messages ADD COLUMN superseded INTEGER NOT NULL DEFAULT 0")

        # Proposal versioning + undo
        if not _column_exists(conn, "sessions", "proposal_version"):
            conn.execute("ALTER TABLE sessions ADD COLUMN proposal_version INTEGER NOT NULL DEFAULT 1")
        if not _column_exists(conn, "sessions", "why_json"):
            conn.execute("ALTER TABLE sessions ADD COLUMN why_json TEXT")

        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS ai_requests (
                id TEXT PRIMARY KEY,
                conversation_id TEXT,
                created_at TEXT NOT NULL,
                model TEXT,
                status TEXT NOT NULL,
                attempt INTEGER NOT NULL DEFAULT 1,
                latency_ms INTEGER,
                tokens_prompt INTEGER,
                tokens_completion INTEGER,
                rate_limit INTEGER,
                rate_remaining INTEGER,
                rate_reset TEXT,
                cancelled INTEGER NOT NULL DEFAULT 0
            )
            """
        )
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS learned_preferences (
                id TEXT PRIMARY KEY,
                key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                evidence TEXT,
                active INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            """
        )
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS approval_events (
                id TEXT PRIMARY KEY,
                batch_id TEXT NOT NULL,
                goal_id TEXT,
                conversation_id TEXT,
                approved_at TEXT NOT NULL,
                undone_at TEXT,
                session_ids_json TEXT NOT NULL DEFAULT '[]'
            )
            """
        )
        # Backfill revision groups for existing messages
        rows = conn.execute(
            "SELECT id FROM messages WHERE revision_group IS NULL OR revision_group = ''"
        ).fetchall()
        for r in rows:
            mid = r["id"] if isinstance(r, sqlite3.Row) else r[0]
            conn.execute("UPDATE messages SET revision_group=? WHERE id=?", (mid, mid))
        conn.commit()
        set_schema_version(conn, 2)
        version = 2

    if version < 3:
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS categories (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                color TEXT NOT NULL DEFAULT '#93c5fd',
                icon TEXT NOT NULL DEFAULT 'circle',
                keywords TEXT NOT NULL DEFAULT '',
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            """
        )
        cols = {r[1] for r in conn.execute("PRAGMA table_info(sessions)").fetchall()}
        if "category_id" not in cols:
            conn.execute("ALTER TABLE sessions ADD COLUMN category_id TEXT")
        conn.commit()
        set_schema_version(conn, 3)
        version = 3

    if version < 4:
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS chat_folders (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                sort_order INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            """
        )
        if not _column_exists(conn, "conversations", "folder_id"):
            conn.execute("ALTER TABLE conversations ADD COLUMN folder_id TEXT")
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_conversations_folder ON conversations(folder_id)"
        )
        conn.commit()
        set_schema_version(conn, 4)
        version = 4

    if version < 5:
        if not _column_exists(conn, "conversations", "sort_order"):
            conn.execute(
                "ALTER TABLE conversations ADD COLUMN sort_order INTEGER NOT NULL DEFAULT 0"
            )
        rows = conn.execute(
            """
            SELECT id FROM conversations
            WHERE deleted_at IS NULL
            ORDER BY updated_at DESC
            """
        ).fetchall()
        for i, r in enumerate(rows):
            cid = r["id"] if isinstance(r, sqlite3.Row) else r[0]
            conn.execute("UPDATE conversations SET sort_order=? WHERE id=?", (i, cid))
        conn.commit()
        set_schema_version(conn, 5)
        version = 5

    if version < 6:
        # Remove used to archive as status='done'. Those rows are leftover deletes,
        # not genuine completions. Hard-delete them (and related records) once.
        now = datetime.now(timezone.utc).isoformat()
        ids = [
            r["id"] if isinstance(r, sqlite3.Row) else r[0]
            for r in conn.execute("SELECT id FROM goals WHERE status='done'").fetchall()
        ]
        for goal_id in ids:
            conn.execute("DELETE FROM sessions WHERE goal_id=?", (goal_id,))
            conn.execute("DELETE FROM approval_events WHERE goal_id=?", (goal_id,))
            conn.execute(
                """
                UPDATE sparks
                SET promoted_goal_id=NULL,
                    status=CASE WHEN status='promoted' THEN 'open' ELSE status END,
                    updated_at=?
                WHERE promoted_goal_id=?
                """,
                (now, goal_id),
            )
            conn.execute("DELETE FROM goals WHERE id=?", (goal_id,))
        conn.commit()
        set_schema_version(conn, 6)
        version = 6

    if version < 7:
        conn.execute(
            """
            CREATE TABLE IF NOT EXISTS turns (
                request_id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                status TEXT NOT NULL,
                user_message TEXT,
                response_json TEXT,
                pending_ops_json TEXT NOT NULL DEFAULT '[]',
                clarifications_json TEXT NOT NULL DEFAULT '[]',
                activity_json TEXT NOT NULL DEFAULT '[]',
                context_categories_json TEXT NOT NULL DEFAULT '[]',
                user_message_id TEXT,
                assistant_message_id TEXT,
                revision_group TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            """
        )
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_turns_conversation ON turns(conversation_id, created_at)"
        )
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_turns_status ON turns(conversation_id, status)"
        )
        row = conn.execute(
            "SELECT value FROM buddy_meta WHERE key='timezone'"
        ).fetchone()
        if not row:
            conn.execute(
                "INSERT INTO buddy_meta (key, value) VALUES ('timezone', 'Europe/London')"
            )
        conn.commit()
        set_schema_version(conn, 7)
        version = 7

    if version < 8:
        if not _column_exists(conn, "turns", "effects_json"):
            conn.execute(
                "ALTER TABLE turns ADD COLUMN effects_json TEXT NOT NULL DEFAULT '{}'"
            )
        conn.commit()
        set_schema_version(conn, 8)
        version = 8

    return version
