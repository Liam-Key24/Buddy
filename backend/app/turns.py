"""Persisted chat turns — idempotency, previews, clarifications, activity."""

from __future__ import annotations

import json
import uuid
from datetime import datetime, timezone
from typing import Any


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


class TurnStore:
    def __init__(self, conn):
        self.conn = conn

    def get(self, request_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            "SELECT * FROM turns WHERE request_id=?", (request_id,)
        ).fetchone()
        return self._row(row) if row else None

    def latest_for_conversation(self, conversation_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            """
            SELECT * FROM turns
            WHERE conversation_id=?
            ORDER BY created_at DESC
            LIMIT 1
            """,
            (conversation_id,),
        ).fetchone()
        return self._row(row) if row else None

    def latest_awaiting_approval(self, conversation_id: str) -> dict[str, Any] | None:
        row = self.conn.execute(
            """
            SELECT * FROM turns
            WHERE conversation_id=? AND status='awaiting_approval'
            ORDER BY created_at DESC
            LIMIT 1
            """,
            (conversation_id,),
        ).fetchone()
        return self._row(row) if row else None

    def try_begin(
        self,
        request_id: str,
        conversation_id: str,
        user_message: str,
    ) -> dict[str, Any] | None:
        """Insert a received turn. Returns None if request_id already exists."""
        now = _now()
        try:
            self.conn.execute(
                """
                INSERT INTO turns (
                    request_id, conversation_id, status, user_message,
                    response_json, pending_ops_json, clarifications_json,
                    activity_json, context_categories_json,
                    created_at, updated_at
                ) VALUES (?, ?, 'received', ?, NULL, '[]', '[]', '[]', '[]', ?, ?)
                """,
                (request_id, conversation_id, user_message, now, now),
            )
            self.conn.commit()
        except Exception:
            return None
        return self.get(request_id)

    def update(self, request_id: str, **fields: Any) -> dict[str, Any] | None:
        if not fields:
            return self.get(request_id)
        allowed = {
            "status",
            "response_json",
            "pending_ops_json",
            "clarifications_json",
            "activity_json",
            "context_categories_json",
            "user_message_id",
            "assistant_message_id",
            "revision_group",
        }
        sets = ["updated_at=?"]
        values: list[Any] = [_now()]
        for key, value in fields.items():
            if key not in allowed:
                continue
            if key.endswith("_json") and not isinstance(value, str):
                value = json.dumps(value)
            sets.append(f"{key}=?")
            values.append(value)
        values.append(request_id)
        self.conn.execute(
            f"UPDATE turns SET {', '.join(sets)} WHERE request_id=?",
            values,
        )
        self.conn.commit()
        return self.get(request_id)

    def executing_for_conversation(self, conversation_id: str, exclude_id: str | None = None) -> bool:
        sql = """
            SELECT 1 FROM turns
            WHERE conversation_id=? AND status IN ('received', 'interpreting', 'committing')
        """
        params: list[Any] = [conversation_id]
        if exclude_id:
            sql += " AND request_id!=?"
            params.append(exclude_id)
        row = self.conn.execute(sql, params).fetchone()
        return row is not None

    @staticmethod
    def _row(row) -> dict[str, Any]:
        data = dict(row)
        for key in (
            "pending_ops_json",
            "clarifications_json",
            "activity_json",
            "context_categories_json",
        ):
            raw = data.get(key) or "[]"
            try:
                data[key.replace("_json", "")] = json.loads(raw)
            except json.JSONDecodeError:
                data[key.replace("_json", "")] = []
        if data.get("response_json"):
            try:
                data["response"] = json.loads(data["response_json"])
            except json.JSONDecodeError:
                data["response"] = None
        else:
            data["response"] = None
        return data


def new_op_id() -> str:
    return str(uuid.uuid4())
