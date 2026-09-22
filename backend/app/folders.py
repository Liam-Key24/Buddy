"""Chat folders — local, zero Cloud AI."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from typing import Any


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class FolderStore:
    def __init__(self, conn):
        self.conn = conn

    def list(self) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            """
            SELECT * FROM chat_folders
            ORDER BY sort_order ASC, created_at ASC
            """
        ).fetchall()
        return [self._row(r) for r in rows]

    def get(self, folder_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            "SELECT * FROM chat_folders WHERE id = ?", (folder_id,)
        ).fetchone()
        return self._row(row) if row else None

    def create(self, title: str) -> dict[str, Any]:
        fid = _new_id()
        now = _now()
        max_order = self.conn.execute(
            "SELECT COALESCE(MAX(sort_order), -1) AS n FROM chat_folders"
        ).fetchone()
        n = max_order["n"] if max_order else -1
        self.conn.execute(
            """
            INSERT INTO chat_folders (id, title, sort_order, created_at, updated_at)
            VALUES (?, ?, ?, ?, ?)
            """,
            (fid, title.strip() or "Untitled", int(n) + 1, now, now),
        )
        self.conn.commit()
        return self.get(fid)

    def rename(self, folder_id: str, title: str) -> dict[str, Any] | None:
        if not self.get(folder_id):
            return None
        self.conn.execute(
            "UPDATE chat_folders SET title=?, updated_at=? WHERE id=?",
            (title.strip() or "Untitled", _now(), folder_id),
        )
        self.conn.commit()
        return self.get(folder_id)

    def delete(self, folder_id: str) -> dict[str, Any] | None:
        row = self.get(folder_id)
        if not row:
            return None
        self.conn.execute(
            "UPDATE conversations SET folder_id=NULL, updated_at=? WHERE folder_id=?",
            (_now(), folder_id),
        )
        self.conn.execute("DELETE FROM chat_folders WHERE id=?", (folder_id,))
        self.conn.commit()
        return row

    def _row(self, row: Any) -> dict[str, Any]:
        d = dict(row)
        return {
            "id": d["id"],
            "title": d.get("title") or "Untitled",
            "sort_order": d.get("sort_order") or 0,
            "created_at": d["created_at"],
            "updated_at": d["updated_at"],
        }
