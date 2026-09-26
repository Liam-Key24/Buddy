"""Local Cloud AI usage accounting — no prompts or secrets."""

from __future__ import annotations

import os
import uuid
from datetime import datetime, timezone
from typing import Any

from .owners import resolve_owner

# Shared Groq key is 1000/day; each private account gets half.
DEFAULT_DAILY_LIMIT = 500


def daily_limit() -> int:
    raw = os.environ.get("BUDDY_DAILY_REQUEST_LIMIT", "").strip()
    if not raw:
        return DEFAULT_DAILY_LIMIT
    try:
        value = int(raw)
    except ValueError:
        return DEFAULT_DAILY_LIMIT
    return max(1, value)


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class UsageStore:
    def __init__(self, conn):
        self.conn = conn

    def _oid(self, owner_user_id: str | None = None) -> str:
        return resolve_owner(self.conn, owner_user_id)

    def record(
        self,
        *,
        conversation_id: str | None,
        model: str | None,
        status: str,
        attempt: int = 1,
        latency_ms: int | None = None,
        tokens_prompt: int | None = None,
        tokens_completion: int | None = None,
        rate_limit: int | None = None,
        rate_remaining: int | None = None,
        rate_reset: str | None = None,
        cancelled: bool = False,
        request_id: str | None = None,
        owner_user_id: str | None = None,
    ) -> str:
        rid = request_id or _new_id()
        self.conn.execute(
            """
            INSERT INTO ai_requests (
                id, conversation_id, created_at, model, status, attempt,
                latency_ms, tokens_prompt, tokens_completion,
                rate_limit, rate_remaining, rate_reset, cancelled, owner_user_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """,
            (
                rid,
                conversation_id,
                _now(),
                model,
                status,
                attempt,
                latency_ms,
                tokens_prompt,
                tokens_completion,
                rate_limit,
                rate_remaining,
                rate_reset,
                1 if cancelled else 0,
                self._oid(owner_user_id),
            ),
        )
        self.conn.commit()
        return rid

    def mark_cancelled(self, request_id: str) -> None:
        self.conn.execute(
            "UPDATE ai_requests SET status='cancelled', cancelled=1 WHERE id=?",
            (request_id,),
        )
        self.conn.commit()

    def today_summary(self, owner_user_id: str | None = None) -> dict[str, Any]:
        today = datetime.now(timezone.utc).date().isoformat()
        rows = self.conn.execute(
            """
            SELECT * FROM ai_requests
            WHERE created_at >= ? AND owner_user_id=?
            ORDER BY created_at DESC
            """,
            (today, self._oid(owner_user_id)),
        ).fetchall()
        used = 0
        last_limit = None
        last_remaining = None
        last_reset = None
        for r in rows:
            d = dict(r)
            # Count every recorded Cloud AI attempt today (incl. retries / failures).
            if d.get("status") in {
                "ok",
                "error",
                "cancelled",
                "rate_limited",
                "malformed",
                "transport",
                "auth",
                "http",
                "config",
            }:
                used += 1
            if d.get("rate_limit") is not None:
                last_limit = d["rate_limit"]
            if d.get("rate_remaining") is not None:
                last_remaining = d["rate_remaining"]
            if d.get("rate_reset"):
                last_reset = d["rate_reset"]
        # Buddy's meter is local attempts per owner. Groq header remaining is a rolling
        # window and must not replace the local used count.
        limit = daily_limit()
        remaining = max(0, limit - used)
        return {
            "used": used,
            "limit": limit,
            "remaining": remaining,
            "reset": last_reset,
            "source": "local_attempts",
            "label": f"Cloud requests {used} / {limit} today",
            "groq_limit": last_limit,
            "groq_remaining": last_remaining,
        }
