"""Local conversation management — zero Cloud AI."""

from __future__ import annotations

import json
import re
import uuid
from datetime import datetime, timezone
from typing import Any


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


def title_from_message(text: str) -> str:
    cleaned = re.sub(r"\s+", " ", text.strip())
    if not cleaned:
        return "New chat"
    if len(cleaned) <= 48:
        return cleaned
    return cleaned[:45].rstrip() + "…"


class ConversationStore:
    def __init__(self, conn):
        self.conn = conn

    def create(self) -> dict[str, Any]:
        cid = _new_id()
        now = _now()
        self.conn.execute(
            """
            INSERT INTO conversations (id, created_at, updated_at, title, deleted_at, draft_json)
            VALUES (?, ?, ?, ?, NULL, '{}')
            """,
            (cid, now, now, "New chat"),
        )
        self.conn.commit()
        return self.get(cid)

    def get(self, conversation_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            "SELECT * FROM conversations WHERE id = ?", (conversation_id,)
        ).fetchone()
        return self._row(row) if row else None

    def list_active(self) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            """
            SELECT * FROM conversations
            WHERE deleted_at IS NULL
            ORDER BY updated_at DESC
            """
        ).fetchall()
        return [self._row(r) for r in rows]

    def list_deleted(self) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            """
            SELECT * FROM conversations
            WHERE deleted_at IS NOT NULL
            ORDER BY deleted_at DESC
            """
        ).fetchall()
        return [self._row(r) for r in rows]

    def rename(self, conversation_id: str, title: str) -> dict[str, Any] | None:
        row = self.get(conversation_id)
        if not row or row.get("deleted_at"):
            return None
        self.conn.execute(
            "UPDATE conversations SET title=?, updated_at=? WHERE id=?",
            (title.strip() or "Untitled", _now(), conversation_id),
        )
        self.conn.commit()
        return self.get(conversation_id)

    def soft_delete(self, conversation_id: str) -> dict[str, Any] | None:
        row = self.get(conversation_id)
        if not row or row.get("deleted_at"):
            return None
        self.conn.execute(
            "UPDATE conversations SET deleted_at=?, updated_at=? WHERE id=?",
            (_now(), _now(), conversation_id),
        )
        self.conn.commit()
        return self.get(conversation_id)

    def restore(self, conversation_id: str) -> dict[str, Any] | None:
        row = self.get(conversation_id)
        if not row or not row.get("deleted_at"):
            return None
        self.conn.execute(
            "UPDATE conversations SET deleted_at=NULL, updated_at=? WHERE id=?",
            (_now(), conversation_id),
        )
        self.conn.commit()
        return self.get(conversation_id)

    def save_draft(self, conversation_id: str, draft: dict[str, Any]) -> dict[str, Any] | None:
        if not self.get(conversation_id):
            return None
        self.conn.execute(
            "UPDATE conversations SET draft_json=?, updated_at=? WHERE id=?",
            (json.dumps(draft), _now(), conversation_id),
        )
        self.conn.commit()
        return self.get(conversation_id)

    def touch_title_from_first_user_message(self, conversation_id: str, message: str) -> None:
        row = self.get(conversation_id)
        if not row:
            return
        if row.get("title") in {None, "", "New chat"}:
            self.rename(conversation_id, title_from_message(message))

    def _row(self, row: Any) -> dict[str, Any]:
        d = dict(row)
        draft_raw = d.get("draft_json") or "{}"
        try:
            draft = json.loads(draft_raw)
        except json.JSONDecodeError:
            draft = {}
        return {
            "id": d["id"],
            "title": d.get("title") or "New chat",
            "created_at": d["created_at"],
            "updated_at": d["updated_at"],
            "deleted_at": d.get("deleted_at"),
            "draft": draft if isinstance(draft, dict) else {},
        }
