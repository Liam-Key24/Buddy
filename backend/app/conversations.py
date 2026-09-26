"""Local conversation management — zero Cloud AI."""

from __future__ import annotations

import json
import re
import uuid
from datetime import datetime, timezone
from typing import Any

from .owners import resolve_owner


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

    def _oid(self, owner_user_id: str | None = None) -> str:
        return resolve_owner(self.conn, owner_user_id)

    def create(self, owner_user_id: str | None = None) -> dict[str, Any]:
        oid = self._oid(owner_user_id)
        cid = _new_id()
        now = _now()
        min_order = self.conn.execute(
            """
            SELECT COALESCE(MIN(sort_order), 1) AS n
            FROM conversations
            WHERE deleted_at IS NULL AND owner_user_id=?
            """,
            (oid,),
        ).fetchone()
        sort_order = int(min_order["n"] if min_order else 1) - 1
        self.conn.execute(
            """
            INSERT INTO conversations (
                id, created_at, updated_at, title, deleted_at, draft_json, sort_order, owner_user_id
            )
            VALUES (?, ?, ?, ?, NULL, '{}', ?, ?)
            """,
            (cid, now, now, "New chat", sort_order, oid),
        )
        self.conn.commit()
        return self.get(cid, owner_user_id=oid)

    def get(self, conversation_id: str, owner_user_id: str | None = None) -> dict[str, Any] | None:
        oid = self._oid(owner_user_id)
        row = self.conn.execute(
            "SELECT * FROM conversations WHERE id = ? AND owner_user_id=?",
            (conversation_id, oid),
        ).fetchone()
        return self._row(row) if row else None

    def list_active(self, owner_user_id: str | None = None) -> list[dict[str, Any]]:
        oid = self._oid(owner_user_id)
        rows = self.conn.execute(
            """
            SELECT c.*, (
                SELECT COUNT(*) FROM messages m
                WHERE m.conversation_id = c.id AND m.role = 'user'
            ) AS user_message_count
            FROM conversations c
            WHERE c.deleted_at IS NULL AND c.owner_user_id=?
            ORDER BY c.sort_order ASC, c.updated_at DESC
            """,
            (oid,),
        ).fetchall()
        return [self._row(r) for r in rows]

    def list_deleted(self, owner_user_id: str | None = None) -> list[dict[str, Any]]:
        oid = self._oid(owner_user_id)
        rows = self.conn.execute(
            """
            SELECT * FROM conversations
            WHERE deleted_at IS NOT NULL AND owner_user_id=?
            ORDER BY deleted_at DESC
            """,
            (oid,),
        ).fetchall()
        return [self._row(r) for r in rows]

    def move(self, conversation_id: str, folder_id: str | None) -> dict[str, Any] | None:
        row = self.get(conversation_id)
        if not row or row.get("deleted_at"):
            return None
        self.conn.execute(
            "UPDATE conversations SET folder_id=?, updated_at=? WHERE id=?",
            (folder_id, _now(), conversation_id),
        )
        self.conn.commit()
        return self.get(conversation_id)

    def place(
        self,
        conversation_id: str,
        folder_id: str | None,
        before_id: str | None = None,
    ) -> dict[str, Any] | None:
        row = self.get(conversation_id)
        if not row or row.get("deleted_at"):
            return None
        self.conn.execute(
            "UPDATE conversations SET folder_id=? WHERE id=?",
            (folder_id, conversation_id),
        )
        if folder_id:
            siblings = self.conn.execute(
                """
                SELECT id FROM conversations
                WHERE deleted_at IS NULL AND folder_id=? AND owner_user_id=?
                ORDER BY sort_order ASC, updated_at DESC
                """,
                (folder_id, self._oid()),
            ).fetchall()
        else:
            siblings = self.conn.execute(
                """
                SELECT id FROM conversations
                WHERE deleted_at IS NULL AND folder_id IS NULL AND owner_user_id=?
                ORDER BY sort_order ASC, updated_at DESC
                """,
                (self._oid(),),
            ).fetchall()
        ids = [r["id"] for r in siblings if r["id"] != conversation_id]
        if before_id and before_id in ids:
            ids.insert(ids.index(before_id), conversation_id)
        else:
            ids.append(conversation_id)
        for i, cid in enumerate(ids):
            self.conn.execute("UPDATE conversations SET sort_order=? WHERE id=?", (i, cid))
        self.conn.commit()
        return self.get(conversation_id)

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
        current = (self.get(conversation_id) or {}).get("draft") or {}
        merged = dict(current) if isinstance(current, dict) else {}
        if isinstance(draft, dict):
            merged.update(draft)
        self.conn.execute(
            "UPDATE conversations SET draft_json=?, updated_at=? WHERE id=?",
            (json.dumps(merged), _now(), conversation_id),
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
            "folder_id": d.get("folder_id"),
            "sort_order": d.get("sort_order") or 0,
            "user_message_count": int(d.get("user_message_count") or 0),
            "draft": draft if isinstance(draft, dict) else {},
        }
