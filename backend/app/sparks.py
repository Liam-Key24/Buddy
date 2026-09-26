"""Sparks: capture ideas without turning them into commitments."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from typing import Any

from .owners import resolve_owner
from .schemas import Spark


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class SparkService:
    def __init__(self, conn):
        self.conn = conn

    def _oid(self, owner_user_id: str | None = None) -> str:
        return resolve_owner(self.conn, owner_user_id)

    def capture(self, content: str, owner_user_id: str | None = None) -> Spark:
        oid = self._oid(owner_user_id)
        sid = _new_id()
        now = _now()
        self.conn.execute(
            """
            INSERT INTO sparks (id, content, status, promoted_goal_id, created_at, updated_at, owner_user_id)
            VALUES (?, ?, 'open', NULL, ?, ?, ?)
            """,
            (sid, content.strip(), now, now, oid),
        )
        self.conn.commit()
        return Spark(id=sid, content=content.strip(), status="open", promoted_goal_id=None, created_at=now)

    def list_open(self, owner_user_id: str | None = None) -> list[Spark]:
        rows = self.conn.execute(
            "SELECT * FROM sparks WHERE status = 'open' AND owner_user_id=? ORDER BY created_at DESC",
            (self._oid(owner_user_id),),
        ).fetchall()
        return [self._row(r) for r in rows]

    def list_all(self, owner_user_id: str | None = None) -> list[Spark]:
        rows = self.conn.execute(
            "SELECT * FROM sparks WHERE owner_user_id=? ORDER BY created_at DESC",
            (self._oid(owner_user_id),),
        ).fetchall()
        return [self._row(r) for r in rows]

    def get(self, spark_id: str, owner_user_id: str | None = None) -> Spark | None:
        row = self.conn.execute(
            "SELECT * FROM sparks WHERE id = ? AND owner_user_id=?",
            (spark_id, self._oid(owner_user_id)),
        ).fetchone()
        return self._row(row) if row else None

    def restore_snapshot(self, snap: dict[str, Any]) -> Spark | None:
        sid = snap.get("id")
        if not sid:
            return None
        now = _now()
        existing = self.conn.execute("SELECT id FROM sparks WHERE id=?", (sid,)).fetchone()
        if existing:
            self.conn.execute(
                """
                UPDATE sparks SET content=?, status=?, promoted_goal_id=?, updated_at=?
                WHERE id=?
                """,
                (
                    snap.get("content") or "",
                    snap.get("status") or "open",
                    snap.get("promoted_goal_id"),
                    now,
                    sid,
                ),
            )
        else:
            self.conn.execute(
                """
                INSERT INTO sparks (id, content, status, promoted_goal_id, created_at, updated_at, owner_user_id)
                VALUES (?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    sid,
                    snap.get("content") or "",
                    snap.get("status") or "open",
                    snap.get("promoted_goal_id"),
                    snap.get("created_at") or now,
                    now,
                    self._oid(),
                ),
            )
        self.conn.commit()
        return self.get(sid)

    def remove(self, spark_id: str, owner_user_id: str | None = None) -> None:
        self.conn.execute(
            "DELETE FROM sparks WHERE id=? AND owner_user_id=?",
            (spark_id, self._oid(owner_user_id)),
        )
        self.conn.commit()

    def promote(self, spark_id: str, goal_id: str, owner_user_id: str | None = None) -> Spark | None:
        if not self.get(spark_id, owner_user_id=owner_user_id):
            return None
        now = _now()
        self.conn.execute(
            """
            UPDATE sparks SET status='promoted', promoted_goal_id=?, updated_at=?
            WHERE id=? AND owner_user_id=?
            """,
            (goal_id, now, spark_id, self._oid(owner_user_id)),
        )
        self.conn.commit()
        return self.get(spark_id, owner_user_id=owner_user_id)

    def dismiss(self, spark_id: str, owner_user_id: str | None = None) -> Spark | None:
        if not self.get(spark_id, owner_user_id=owner_user_id):
            return None
        now = _now()
        self.conn.execute(
            "UPDATE sparks SET status='dismissed', updated_at=? WHERE id=? AND owner_user_id=?",
            (now, spark_id, self._oid(owner_user_id)),
        )
        self.conn.commit()
        return self.get(spark_id, owner_user_id=owner_user_id)

    def _row(self, row: Any) -> Spark:
        d = dict(row)
        return Spark(
            id=d["id"],
            content=d["content"],
            status=d["status"],
            promoted_goal_id=d.get("promoted_goal_id"),
            created_at=d["created_at"],
        )
