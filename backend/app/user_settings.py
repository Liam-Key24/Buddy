"""Per-user settings JSON and calendar block sync."""

from __future__ import annotations

import copy
import json
import uuid
from datetime import date, datetime, timezone
from typing import Any

import sqlite3

DEFAULT_SETTINGS: dict[str, Any] = {
    "show_avatar": True,
    "compact_sidebar": False,
    "confirm_deletes": True,
    "sleep_enabled": True,
    "skip_weekends": True,
    "prefer_after": "17:30",
    "work": {
        "enabled": True,
        "mode": "full_time",
        "start": "09:00",
        "end": "17:00",
        "days": [True, True, True, True, True, False, False],
        "shifts": [],
    },
}

_TOP_KEYS = frozenset(DEFAULT_SETTINGS.keys())
_WORK_KEYS = frozenset(DEFAULT_SETTINGS["work"].keys())


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _hhmm_to_minutes(value: str) -> int:
    parts = str(value).strip().split(":")
    if len(parts) != 2:
        raise ValueError(f"Invalid time: {value}")
    hour, minute = int(parts[0]), int(parts[1])
    if not (0 <= hour <= 24 and 0 <= minute <= 59):
        raise ValueError(f"Invalid time: {value}")
    total = hour * 60 + minute
    if total > 24 * 60:
        raise ValueError(f"Invalid time: {value}")
    return total


def _merge_defaults(stored: dict[str, Any] | None) -> dict[str, Any]:
    out = copy.deepcopy(DEFAULT_SETTINGS)
    if not stored:
        return out
    for key in _TOP_KEYS:
        if key not in stored:
            continue
        if key == "work" and isinstance(stored["work"], dict):
            work = dict(out["work"])
            for wk in _WORK_KEYS:
                if wk in stored["work"]:
                    work[wk] = copy.deepcopy(stored["work"][wk])
            out["work"] = work
        else:
            out[key] = copy.deepcopy(stored[key])
    return out


def _apply_patch(base: dict[str, Any], patch: dict[str, Any]) -> dict[str, Any]:
    out = copy.deepcopy(base)
    for key, value in patch.items():
        if key not in _TOP_KEYS:
            continue
        if key == "work":
            if not isinstance(value, dict):
                continue
            work = dict(out["work"])
            for wk, wv in value.items():
                if wk in _WORK_KEYS:
                    work[wk] = copy.deepcopy(wv)
            out["work"] = work
        else:
            out[key] = copy.deepcopy(value)
    return out


def get_settings(conn: sqlite3.Connection, owner_user_id: str) -> dict[str, Any]:
    row = conn.execute(
        "SELECT settings_json FROM user_settings WHERE owner_user_id=?",
        (owner_user_id,),
    ).fetchone()
    if not row:
        return _merge_defaults(None)
    raw = row["settings_json"] if isinstance(row, sqlite3.Row) else row[0]
    try:
        stored = json.loads(raw) if isinstance(raw, str) else {}
    except json.JSONDecodeError:
        stored = {}
    if not isinstance(stored, dict):
        stored = {}
    return _merge_defaults(stored)


def put_settings(
    conn: sqlite3.Connection, owner_user_id: str, patch: dict[str, Any]
) -> dict[str, Any]:
    current = get_settings(conn, owner_user_id)
    merged = _apply_patch(current, patch if isinstance(patch, dict) else {})
    now = _now()
    conn.execute(
        """
        INSERT INTO user_settings (owner_user_id, settings_json, updated_at)
        VALUES (?, ?, ?)
        ON CONFLICT(owner_user_id) DO UPDATE SET
            settings_json = excluded.settings_json,
            updated_at = excluded.updated_at
        """,
        (owner_user_id, json.dumps(merged), now),
    )
    _sync_calendar_blocks(conn, owner_user_id, merged)
    conn.commit()
    return merged


def _insert_block(
    conn: sqlite3.Connection,
    *,
    owner_user_id: str,
    title: str,
    weekday: int,
    start_minute: int,
    end_minute: int,
    created_at: str,
) -> None:
    conn.execute(
        """
        INSERT INTO fixed_blocks (id, title, weekday, start_minute, end_minute, created_at, owner_user_id)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        """,
        (
            str(uuid.uuid4()),
            title,
            weekday,
            start_minute,
            end_minute,
            created_at,
            owner_user_id,
        ),
    )


def _sync_calendar_blocks(
    conn: sqlite3.Connection, owner_user_id: str, settings: dict[str, Any]
) -> None:
    now = _now()
    conn.execute(
        """
        DELETE FROM fixed_blocks
        WHERE owner_user_id=?
          AND (title = 'Work' OR title LIKE 'Work · %')
        """,
        (owner_user_id,),
    )

    work = settings.get("work") or {}
    if work.get("enabled"):
        mode = work.get("mode") or "full_time"
        if mode == "full_time":
            start_m = _hhmm_to_minutes(str(work.get("start") or "09:00"))
            end_m = _hhmm_to_minutes(str(work.get("end") or "17:00"))
            days = work.get("days") or []
            for weekday, on in enumerate(days):
                if not on:
                    continue
                if not (0 <= weekday <= 6):
                    continue
                _insert_block(
                    conn,
                    owner_user_id=owner_user_id,
                    title="Work",
                    weekday=weekday,
                    start_minute=start_m,
                    end_minute=end_m,
                    created_at=now,
                )
        elif mode == "part_time":
            for shift in work.get("shifts") or []:
                if not isinstance(shift, dict):
                    continue
                date_s = str(shift.get("date") or "").strip()
                if not date_s:
                    continue
                weekday = date.fromisoformat(date_s).weekday()
                start_m = _hhmm_to_minutes(str(shift.get("start") or "09:00"))
                end_m = _hhmm_to_minutes(str(shift.get("end") or "17:00"))
                place = str(shift.get("place") or "").strip()
                title = f"Work · {place}" if place else "Work"
                _insert_block(
                    conn,
                    owner_user_id=owner_user_id,
                    title=title,
                    weekday=weekday,
                    start_minute=start_m,
                    end_minute=end_m,
                    created_at=now,
                )

    if not settings.get("sleep_enabled", True):
        conn.execute(
            """
            DELETE FROM fixed_blocks
            WHERE owner_user_id=?
              AND title IN ('Sleep', 'Sleep early')
            """,
            (owner_user_id,),
        )
    else:
        row = conn.execute(
            """
            SELECT COUNT(*) AS c FROM fixed_blocks
            WHERE owner_user_id=? AND title IN ('Sleep', 'Sleep early')
            """,
            (owner_user_id,),
        ).fetchone()
        count = row["c"] if isinstance(row, sqlite3.Row) else row[0]
        if not count:
            for weekday in range(0, 7):
                _insert_block(
                    conn,
                    owner_user_id=owner_user_id,
                    title="Sleep",
                    weekday=weekday,
                    start_minute=23 * 60,
                    end_minute=24 * 60,
                    created_at=now,
                )
                _insert_block(
                    conn,
                    owner_user_id=owner_user_id,
                    title="Sleep early",
                    weekday=weekday,
                    start_minute=0,
                    end_minute=7 * 60,
                    created_at=now,
                )
