"""Sparks: capture ideas without turning them into commitments."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from typing import Any

from .schemas import Spark


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class SparkService:
    def __init__(self, conn):
        self.conn = conn

    def capture(self, content: str) -> Spark:
        sid = _new_id()
        now = _now()
        self.conn.execute(
            """
            INSERT INTO sparks (id, content, status, promoted_goal_id, created_at, updated_at)
            VALUES (?, ?, 'open', NULL, ?, ?)
            """,
            (sid, content.strip(), now, now),
        )
        self.conn.commit()
        return Spark(id=sid, content=content.strip(), status="open", promoted_goal_id=None, created_at=now)

    def list_open(self) -> list[Spark]:
        rows = self.conn.execute(
            "SELECT * FROM sparks WHERE status = 'open' ORDER BY created_at DESC"
        ).fetchall()
        return [self._row(r) for r in rows]

    def list_all(self) -> list[Spark]:
        rows = self.conn.execute("SELECT * FROM sparks ORDER BY created_at DESC").fetchall()
        return [self._row(r) for r in rows]

    def promote(self, spark_id: str, goal_id: str) -> Spark | None:
        row = self.conn.execute("SELECT * FROM sparks WHERE id = ?", (spark_id,)).fetchone()
        if not row:
            return None
        now = _now()
        self.conn.execute(
            """
            UPDATE sparks SET status='promoted', promoted_goal_id=?, updated_at=? WHERE id=?
            """,
            (goal_id, now, spark_id),
        )
        self.conn.commit()
        refreshed = self.conn.execute("SELECT * FROM sparks WHERE id = ?", (spark_id,)).fetchone()
        return self._row(refreshed)

    def dismiss(self, spark_id: str) -> Spark | None:
        row = self.conn.execute("SELECT * FROM sparks WHERE id = ?", (spark_id,)).fetchone()
        if not row:
            return None
        now = _now()
        self.conn.execute(
            "UPDATE sparks SET status='dismissed', updated_at=? WHERE id=?",
            (now, spark_id),
        )
        self.conn.commit()
        refreshed = self.conn.execute("SELECT * FROM sparks WHERE id = ?", (spark_id,)).fetchone()
        return self._row(refreshed)

    def _row(self, row: Any) -> Spark:
        d = dict(row)
        return Spark(
            id=d["id"],
            content=d["content"],
            status=d["status"],
            promoted_goal_id=d.get("promoted_goal_id"),
            created_at=d["created_at"],
        )
