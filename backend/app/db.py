from __future__ import annotations

import os
import sqlite3
from contextlib import contextmanager
from pathlib import Path
from typing import Iterator

from .migrations import run_migrations

DEFAULT_DB_PATH = Path(__file__).resolve().parents[1] / "data" / "buddy.db"
_TXN_DEPTH: dict[int, int] = {}


def resolve_db_path(db_path: Path | None = None) -> Path:
    if db_path is not None:
        return db_path
    env = os.environ.get("BUDDY_DB_PATH", "").strip()
    if env:
        return Path(env).expanduser()
    return DEFAULT_DB_PATH


def get_connection(db_path: Path | None = None) -> sqlite3.Connection:
    path = resolve_db_path(db_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA journal_mode = WAL")
    conn.execute("PRAGMA busy_timeout = 5000")
    return conn


def _txn_depth(conn: sqlite3.Connection) -> int:
    return _TXN_DEPTH.get(id(conn), 0)


def commit(conn: sqlite3.Connection) -> None:
    """Skip SQLite commit while a managed transaction is open."""
    if _txn_depth(conn):
        return
    sqlite3.Connection.commit(conn)


@contextmanager
def transaction(conn: sqlite3.Connection) -> Iterator[None]:
    """One BEGIN IMMEDIATE / COMMIT for a batch of store writes."""
    key = id(conn)
    depth = _txn_depth(conn)
    _TXN_DEPTH[key] = depth + 1
    outer = depth == 0
    if outer and not conn.in_transaction:
        conn.execute("BEGIN IMMEDIATE")
    try:
        yield
        if outer:
            sqlite3.Connection.commit(conn)
    except Exception:
        if outer:
            conn.rollback()
        raise
    finally:
        if depth:
            _TXN_DEPTH[key] = depth
        else:
            _TXN_DEPTH.pop(key, None)


def init_db(conn: sqlite3.Connection) -> None:
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS conversations (
            id TEXT PRIMARY KEY,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS messages (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS goals (
            id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            title TEXT NOT NULL,
            domain TEXT,
            target TEXT,
            deadline TEXT,
            baseline TEXT,
            frequency TEXT,
            commitment TEXT,
            status TEXT NOT NULL DEFAULT 'gathering',
            facts_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (conversation_id) REFERENCES conversations(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS fixed_blocks (
            id TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            weekday INTEGER NOT NULL,
            start_minute INTEGER NOT NULL,
            end_minute INTEGER NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            goal_id TEXT,
            title TEXT NOT NULL,
            start_at TEXT NOT NULL,
            end_at TEXT NOT NULL,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            proposal_batch_id TEXT,
            notes TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (goal_id) REFERENCES goals(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS sparks (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'open',
            promoted_goal_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY (promoted_goal_id) REFERENCES goals(id) ON DELETE SET NULL
        );

        CREATE INDEX IF NOT EXISTS idx_messages_conversation
            ON messages(conversation_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_goals_conversation
            ON goals(conversation_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_start
            ON sessions(start_at);
        CREATE INDEX IF NOT EXISTS idx_sessions_goal
            ON sessions(goal_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_batch
            ON sessions(proposal_batch_id);
        CREATE INDEX IF NOT EXISTS idx_sparks_status
            ON sparks(status);
        """
    )
    conn.commit()
    run_migrations(conn)
    _seed_default_fixed_blocks(conn)


def seed_default_fixed_blocks(conn: sqlite3.Connection, owner_user_id: str) -> None:
    row = conn.execute(
        "SELECT COUNT(*) AS c FROM fixed_blocks WHERE owner_user_id=?",
        (owner_user_id,),
    ).fetchone()
    if row and row["c"] > 0:
        return
    import uuid
    from datetime import datetime, timezone

    now = datetime.now(timezone.utc).isoformat()
    blocks = []
    for weekday in range(0, 5):
        blocks.append(("Work", weekday, 9 * 60, 17 * 60))
    for weekday in range(0, 7):
        blocks.append(("Sleep", weekday, 23 * 60, 24 * 60))
        blocks.append(("Sleep early", weekday, 0, 7 * 60))
    for title, weekday, start_m, end_m in blocks:
        conn.execute(
            """
            INSERT INTO fixed_blocks (id, title, weekday, start_minute, end_minute, created_at, owner_user_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            """,
            (str(uuid.uuid4()), title, weekday, start_m, end_m, now, owner_user_id),
        )
    conn.commit()


def _seed_default_fixed_blocks(conn: sqlite3.Connection) -> None:
    tables = {r[0] for r in conn.execute("SELECT name FROM sqlite_master WHERE type='table'").fetchall()}
    if "users" not in tables:
        return
    from .auth import list_users

    for user in list_users(conn):
        seed_default_fixed_blocks(conn, user["id"])
