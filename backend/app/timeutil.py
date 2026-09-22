"""User timezone helpers. Default Europe/London, including BST."""

from __future__ import annotations

from datetime import datetime, timedelta
from zoneinfo import ZoneInfo

DEFAULT_TIMEZONE = "Europe/London"


def zone(name: str | None) -> ZoneInfo:
    try:
        return ZoneInfo(name or DEFAULT_TIMEZONE)
    except Exception:
        return ZoneInfo(DEFAULT_TIMEZONE)


def local_now(tz_name: str | None, *, at: datetime | None = None) -> datetime:
    tz = zone(tz_name)
    if at is None:
        return datetime.now(tz)
    if at.tzinfo is None:
        return at.replace(tzinfo=tz)
    return at.astimezone(tz)


def local_today(tz_name: str | None, *, at: datetime | None = None) -> str:
    return local_now(tz_name, at=at).date().isoformat()


def resolve_relative_date(
    phrase: str,
    tz_name: str | None = DEFAULT_TIMEZONE,
    *,
    at: datetime | None = None,
) -> str | None:
    """Map today/tomorrow/tonight/weekend/next Monday into a local YYYY-MM-DD."""
    now = local_now(tz_name, at=at)
    lower = phrase.strip().lower()
    if lower in {"today", "tonight", "this evening", "after work"}:
        return now.date().isoformat()
    if lower == "tomorrow":
        return (now.date() + timedelta(days=1)).isoformat()
    if lower in {"weekend", "this weekend"}:
        # Next Saturday (or today if Saturday/Sunday).
        delta = (5 - now.weekday()) % 7
        return (now.date() + timedelta(days=delta)).isoformat()
    if lower in {"next monday", "monday"}:
        delta = (0 - now.weekday()) % 7
        if delta == 0:
            delta = 7 if lower.startswith("next") else 0
        return (now.date() + timedelta(days=delta)).isoformat()
    return None
