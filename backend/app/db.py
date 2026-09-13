from __future__ import annotations

import sqlite3
from pathlib import Path

DEFAULT_DB_PATH = Path(__file__).resolve().parents[1] / "data" / "buddy.db"


def get_connection(db_path: Path | None = None) -> sqlite3.Connection:
    path = db_path or DEFAULT_DB_PATH
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(str(path), check_same_thread=False)
    conn.row_factory = sqlite3.Row
    conn.execute("PRAGMA foreign_keys = ON")
    return conn


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
    _seed_default_fixed_blocks(conn)


def _seed_default_fixed_blocks(conn: sqlite3.Connection) -> None:
    row = conn.execute("SELECT COUNT(*) AS c FROM fixed_blocks").fetchone()
    if row and row["c"] > 0:
        return
    # Weekday work + nightly sleep as protected fixed commitments.
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
            INSERT INTO fixed_blocks (id, title, weekday, start_minute, end_minute, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            """,
            (str(uuid.uuid4()), title, weekday, start_m, end_m, now),
        )
    conn.commit()
