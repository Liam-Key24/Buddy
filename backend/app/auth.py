"""Private couple accounts — username/password, HttpOnly session cookie."""

from __future__ import annotations

import hashlib
import os
import secrets
import sqlite3
import uuid
from datetime import datetime, timedelta, timezone
from typing import Any

from argon2 import PasswordHasher
from argon2.exceptions import VerifyMismatchError

from .owners import reset_owner, set_owner

SESSION_COOKIE = "buddy_session"
SESSION_DAYS = 30
_hasher = PasswordHasher()


def _now() -> datetime:
    return datetime.now(timezone.utc)


def _now_iso() -> str:
    return _now().isoformat()


def parse_user_spec(raw: str) -> tuple[str, str] | None:
    text = (raw or "").strip()
    if not text or ":" not in text:
        return None
    username, password = text.split(":", 1)
    username = username.strip()
    if not username or not password:
        return None
    return username, password


def bootstrap_user_specs() -> list[tuple[str, str]]:
    specs: list[tuple[str, str]] = []
    seen: set[str] = set()
    for key in ("BUDDY_USER_1", "BUDDY_USER_2"):
        parsed = parse_user_spec(os.environ.get(key, ""))
        if not parsed:
            continue
        username, password = parsed
        lowered = username.lower()
        if lowered in seen:
            continue
        seen.add(lowered)
        specs.append((username, password))
    return specs


def hash_password(password: str) -> str:
    return _hasher.hash(password)


def verify_password(password_hash: str, password: str) -> bool:
    try:
        return _hasher.verify(password_hash, password)
    except (VerifyMismatchError, ValueError):
        return False


def session_secret() -> str:
    return os.environ.get("BUDDY_SESSION_SECRET", "").strip()


def cookie_secure() -> bool:
    raw = os.environ.get("BUDDY_COOKIE_SECURE", "").strip().lower()
    if raw:
        return raw in {"1", "true", "yes", "on"}
    return False


def hash_session_token(raw: str) -> str:
    return hashlib.sha256(f"{session_secret()}:{raw}".encode()).hexdigest()


def new_session_token() -> str:
    return secrets.token_urlsafe(32)


def list_users(conn: sqlite3.Connection) -> list[dict[str, Any]]:
    rows = conn.execute(
        "SELECT id, username, created_at FROM users ORDER BY created_at ASC, username ASC"
    ).fetchall()
    return [dict(r) for r in rows]


def get_user_by_id(conn: sqlite3.Connection, user_id: str) -> dict[str, Any] | None:
    row = conn.execute(
        "SELECT id, username, created_at FROM users WHERE id=?",
        (user_id,),
    ).fetchone()
    return dict(row) if row else None


def get_user_by_username(conn: sqlite3.Connection, username: str) -> dict[str, Any] | None:
    row = conn.execute(
        "SELECT id, username, password_hash, created_at FROM users WHERE username=?",
        (username,),
    ).fetchone()
    return dict(row) if row else None


def create_user(conn: sqlite3.Connection, username: str, password: str) -> dict[str, Any]:
    uid = str(uuid.uuid4())
    conn.execute(
        "INSERT INTO users (id, username, password_hash, created_at) VALUES (?, ?, ?, ?)",
        (uid, username, hash_password(password), _now_iso()),
    )
    conn.commit()
    user = get_user_by_id(conn, uid)
    assert user is not None
    return user


def bootstrap_users(conn: sqlite3.Connection) -> list[dict[str, Any]]:
    """Create seeded accounts when users is empty. Fail if env is missing."""
    existing = list_users(conn)
    if existing:
        return existing
    specs = bootstrap_user_specs()
    if not specs:
        raise RuntimeError(
            "No users in the database and bootstrap env is missing. "
            "Set BUDDY_USER_1=username:password and BUDDY_USER_2=username:password."
        )
    for username, password in specs:
        create_user(conn, username, password)
    return list_users(conn)


def migrate_owner_id(conn: sqlite3.Connection) -> str:
    users = list_users(conn)
    if not users:
        raise RuntimeError("Cannot assign owner_user_id: users table is empty")
    name = os.environ.get("BUDDY_MIGRATE_OWNER", "").strip()
    if name:
        row = conn.execute("SELECT id FROM users WHERE username=?", (name,)).fetchone()
        if not row:
            raise RuntimeError(f"BUDDY_MIGRATE_OWNER={name!r} does not match a seeded user")
        return row["id"]
    return users[0]["id"]


def create_session(conn: sqlite3.Connection, user_id: str, *, days: int = SESSION_DAYS) -> str:
    raw = new_session_token()
    sid = str(uuid.uuid4())
    expires = (_now() + timedelta(days=days)).isoformat()
    conn.execute(
        """
        INSERT INTO auth_sessions (id, user_id, token_hash, expires_at, created_at)
        VALUES (?, ?, ?, ?, ?)
        """,
        (sid, user_id, hash_session_token(raw), expires, _now_iso()),
    )
    conn.commit()
    return raw


def lookup_session(conn: sqlite3.Connection, raw_token: str | None) -> dict[str, Any] | None:
    if not raw_token:
        return None
    row = conn.execute(
        """
        SELECT u.id, u.username, u.created_at, s.id AS session_id, s.expires_at
        FROM auth_sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.token_hash=?
        """,
        (hash_session_token(raw_token),),
    ).fetchone()
    if not row:
        return None
    try:
        expires = datetime.fromisoformat(row["expires_at"])
    except ValueError:
        return None
    if expires.tzinfo is None:
        expires = expires.replace(tzinfo=timezone.utc)
    if expires <= _now():
        conn.execute("DELETE FROM auth_sessions WHERE id=?", (row["session_id"],))
        conn.commit()
        return None
    return {"id": row["id"], "username": row["username"], "created_at": row["created_at"]}


def revoke_session(conn: sqlite3.Connection, raw_token: str | None) -> None:
    if not raw_token:
        return
    conn.execute(
        "DELETE FROM auth_sessions WHERE token_hash=?",
        (hash_session_token(raw_token),),
    )
    conn.commit()


def authenticate(conn: sqlite3.Connection, username: str, password: str) -> dict[str, Any] | None:
    user = get_user_by_username(conn, username.strip())
    if not user or not verify_password(user["password_hash"], password):
        return None
    return {"id": user["id"], "username": user["username"], "created_at": user["created_at"]}


def cookie_kwargs() -> dict[str, Any]:
    return {
        "key": SESSION_COOKIE,
        "httponly": True,
        "secure": cookie_secure(),
        "samesite": "lax",
        "path": "/",
        "max_age": SESSION_DAYS * 24 * 60 * 60,
    }


def bind_request_owner(user_id: str):
    return set_owner(user_id)


def unbind_request_owner(token) -> None:
    reset_owner(token)
