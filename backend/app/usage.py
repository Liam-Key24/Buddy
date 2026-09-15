"""Local Cloud AI usage accounting — no prompts or secrets."""

from __future__ import annotations

import uuid
from datetime import datetime, timezone
from typing import Any


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


class UsageStore:
    def __init__(self, conn):
        self.conn = conn

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
    ) -> str:
        rid = request_id or _new_id()
        self.conn.execute(
            """
            INSERT INTO ai_requests (
                id, conversation_id, created_at, model, status, attempt,
                latency_ms, tokens_prompt, tokens_completion,
                rate_limit, rate_remaining, rate_reset, cancelled
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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

    def today_summary(self) -> dict[str, Any]:
        today = datetime.now(timezone.utc).date().isoformat()
        rows = self.conn.execute(
            """
            SELECT * FROM ai_requests
            WHERE created_at >= ?
            ORDER BY created_at DESC
            """,
            (today,),
        ).fetchall()
        used = 0
        last_limit = None
        last_remaining = None
        last_reset = None
        for r in rows:
            d = dict(r)
            # Count real outbound attempts (including retries and cancelled)
            if d.get("status") in {"ok", "error", "cancelled", "rate_limited", "malformed"}:
                used += 1
            if d.get("rate_limit") is not None:
                last_limit = d["rate_limit"]
            if d.get("rate_remaining") is not None:
                last_remaining = d["rate_remaining"]
            if d.get("rate_reset"):
                last_reset = d["rate_reset"]
        limit = last_limit if last_limit is not None else 1000
        remaining = last_remaining if last_remaining is not None else max(0, limit - used)
        source = "groq_headers" if last_limit is not None else "local_attempts"
        return {
            "used": used if last_remaining is None else max(0, limit - remaining),
            "limit": limit,
            "remaining": remaining,
            "reset": last_reset,
            "source": source,
            "label": f"Cloud requests {used if last_remaining is None else max(0, limit - remaining)} / {limit} today",
        }
