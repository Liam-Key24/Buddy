"""Request-scoped personal-data owner.

Routes bind the authenticated user. Stores resolve owner from an explicit
argument or the request context. The first-user fallback is tests only.
"""

from __future__ import annotations

import os
from contextlib import contextmanager
from contextvars import ContextVar, Token
from typing import Iterator

_owner_user_id: ContextVar[str | None] = ContextVar("buddy_owner_user_id", default=None)


def current_owner_id() -> str | None:
    return _owner_user_id.get()


def set_owner(user_id: str) -> Token:
    return _owner_user_id.set(user_id)


def reset_owner(token: Token) -> None:
    _owner_user_id.reset(token)


@contextmanager
def owner_scope(user_id: str) -> Iterator[str]:
    token = set_owner(user_id)
    try:
        yield user_id
    finally:
        reset_owner(token)


def first_user_id(conn) -> str | None:
    row = conn.execute(
        "SELECT id FROM users ORDER BY created_at ASC, username ASC LIMIT 1"
    ).fetchone()
    if not row:
        return None
    return row["id"] if hasattr(row, "keys") else row[0]


def owner_fallback_allowed() -> bool:
    raw = os.environ.get("BUDDY_OWNER_FALLBACK", "").strip().lower()
    return raw in {"1", "true", "yes", "on"}


def resolve_owner(conn, explicit: str | None = None) -> str:
    if explicit:
        return explicit
    ctx = current_owner_id()
    if ctx:
        return ctx
    if owner_fallback_allowed():
        fallback = first_user_id(conn)
        if fallback:
            return fallback
    raise RuntimeError("owner_user_id required")
