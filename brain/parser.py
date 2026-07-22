import json
import re
from datetime import datetime, timedelta
from typing import Any, Optional

from pydantic import BaseModel, ValidationError


class PreferenceDetected(BaseModel):
    key: str
    value: str
    confidence: float
    source: str


class DecisionDetected(BaseModel):
    decision: str
    reason: str


class PlanResponse(BaseModel):
    intent: str
    tool: Optional[str] = None
    tool_input: Optional[str] = None
    reasoning: str
    response: Optional[str] = None
    task_state: Optional[str] = None
    mode_hint: Optional[str] = None
    # Buddy routes this generically: "passthrough" skips /chat/respond (Brain decides).
    respond_mode: Optional[str] = None
    preference_detected: Optional[PreferenceDetected] = None
    decision_detected: Optional[DecisionDetected] = None


# Tools whose Core output is already user-facing — skip second MLX call.
_PASSTHROUGH_TOOLS = frozenset(
    {
        "echo",
        "save_spark",
        "update_spark",
        "memory.handover",
        "memory.maintain",
        "send_email",
        "git_push",
    }
)
_PASSTHROUGH_PREFIXES = ("calendar.", "dream.", "work.")
# Prefer MLX planning for these even when heuristics match.
_HEURISTIC_SLOW_TOOLS = frozenset({"coder.run"})


def default_respond_mode(tool: Optional[str]) -> str:
    """Brain policy: which tools need an LLM narration after Core."""
    if not tool:
        return "llm"
    if tool in _PASSTHROUGH_TOOLS:
        return "passthrough"
    if any(tool.startswith(p) for p in _PASSTHROUGH_PREFIXES):
        return "passthrough"
    return "llm"


def apply_respond_mode(plan: PlanResponse) -> PlanResponse:
    """Fill respond_mode when missing; never invent tool-specific Buddy logic."""
    mode = (plan.respond_mode or "").strip().lower()
    if mode not in ("passthrough", "llm"):
        mode = default_respond_mode(plan.tool if plan.intent == "tool_use" else None)
    plan.respond_mode = mode
    return plan


def try_fast_heuristic_plan(message: str) -> Optional[PlanResponse]:
    """Confident tool plans that can skip MLX. Returns None for chat / ambiguous."""
    plan = normalize_plan(_heuristic_plan(message))
    if plan.intent != "tool_use" or not plan.tool:
        return None
    if plan.tool in _HEURISTIC_SLOW_TOOLS:
        return None
    # Chat fallback from heuristics always says MLX unavailable — not a fast path.
    if plan.reasoning and "MLX unavailable" in plan.reasoning and plan.tool is None:
        return None
    return apply_respond_mode(plan)

IDEA_TRIGGERS = [
    r"\bi have an idea\b",
    r"\bidea for\b",
    r"\bwhat if we\b",
    r"\bwhat if i\b",
    r"\bthought about\b",
    r"\bthinking about\b",
    r"\bnote to self\b",
    r"\bremember to\b",
    r"\bdon'?t forget\b",
    r"\bmaybe we could\b",
    r"\bmaybe i could\b",
    r"\bwould be (?:cool|nice|good) if\b",
    r"\bi('ve| have) been thinking\b",
    r"\bsomething to (?:try|do|look into)\b",
    r"\bspark:\s*",
    r"\bspark\s+\w",
]

# Heuristic-only triggers for when MLX is unreachable; the real planner
# decides "code" intent from full context, not just these phrases.
CODE_TRIGGERS = [
    r"\bcan you code\b",
    r"\bwrite (?:a|me a|some) (?:script|function|program|app)\b",
    r"\bimplement\b",
    r"\bfix (?:this|the) bug\b",
    r"\bdebug (?:this|my)\b",
    r"\brefactor\b",
    r"\bbuild (?:a|me a) (?:script|app|tool)\b",
]

CALENDAR_QUERY_TODAY = [
    r"\bwhat(?:'s| is) on (?:my )?(?:calendar )?today\b",
    r"\btoday'?s (?:schedule|agenda|events|calendar)\b",
    r"\bshow (?:me )?today\b",
    r"\bdo i have anything today\b",
]

CALENDAR_QUERY_TOMORROW = [
    r"\bwhat(?:'s| is) on (?:my )?(?:calendar )?tomorrow\b",
    r"\btomorrow'?s (?:schedule|agenda|events|calendar)\b",
]

CALENDAR_QUERY_WEEK = [
    r"\bwhat(?:'s| is) (?:on )?(?:my )?(?:calendar )?this week\b",
    r"\bthis week'?s (?:schedule|agenda|events|calendar)\b",
]

CALENDAR_FREE_TIME = [
    r"\bwhen am i free\b",
    r"\bfind (?:me )?(?:\d+\s*(?:hours?|hrs?|minutes?|mins?)|free time|a (?:free )?slot)\b",
    r"\b(?:am i|do i have) (?:any )?free\b",
    r"\bwhat(?:'s| is) (?:my )?availability\b",
    r"\bfree (?:tomorrow|today|this week)\b",
]

CALENDAR_BLOCK_TIME = [
    r"\bblock\b.+\b(?:hours?|hrs?|time|for)\b",
    r"\b(?:focus|deep work)\b.+\b(?:hours?|block)\b",
    r"\bblock\b.+\b(?:coding|writing|focus)\b",
]

CALENDAR_SCHEDULE_TASK = [
    r"\bfinish\b.+\b(?:this week|today|tomorrow)\b",
    r"\bschedule (?:the |a |my )?(?:task|report|project)\b",
    r"\bauto[- ]?schedule\b",
]

CALENDAR_PLAN_DAY = [
    r"\bplan my day\b",
    r"\bplan (?:out )?(?:my |the )?day\b",
    r"\bschedule my day\b",
]

CALENDAR_CAPACITY = [
    r"\b(?:what(?:'s| is)|how(?:'s| is)) (?:my )?(?:capacity|workload)\b",
    r"\bcapacity (?:today|tomorrow)\b",
    r"\bhow(?:'s| is) (?:my )?day looking\b",
    r"\bdaily (?:summary|capacity)\b",
    r"\bsummar(?:y|ise|ize) (?:my )?(?:day|today)\b",
]

CALENDAR_CREATE = [
    r"\b(?:add|create|schedule|book|put)\b.+\b(?:event|meeting|appointment|call|reminder)\b",
    r"\b(?:add|put|schedule)\b.+\b(?:on|to)\b.+\bcalend(?:a|e)r\b",
    r"\badd (?:this|that|it) to (?:my )?(?:calend(?:a|e)r)\b",
    r"\bremind me (?:to|about)\b",
    r"\bevery\s+(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
    r"\b(?:climbing|gym|workout|training|work)\b.+\b(?:hour|am|pm|monday|tuesday|wednesday|thursday|friday|saturday|sunday|mon|tue|wed|thu|fri|sat|sun)\b",
    r"\b\d{1,2}:\d{2}\s*(?:am|pm)?\s*(?:-|to)\s*\d{1,2}:\d{2}",
    r"\bmon(?:day)?\s*(?:till|to|through|-)\s*fri",
]

TOOL_ALIASES = {
    "calendar.add_event": "calendar.create_event",
    "calendar.schedule_event": "calendar.create_event",
    "calendar.add": "calendar.create_event",
    "calendar.schedule": "calendar.create_event",
    "calendar.create": "calendar.create_event",
}

CALENDAR_DELETE = [
    r"\b(?:delete|remove|cancel)\b.+\b(?:events?|meetings?|appointments?)\b",
    r"\b(?:delete|remove|cancel)\b.+\b(?:from )?(?:my )?calend(?:a|e)r\b",
    r"\b(?:clear|wipe|empty)\b.+\b(?:calendar|schedule|events?)\b",
    r"\bremove all\b",
    r"\bclear (?:my )?calendar\b",
    # Short title deletes: "remove gym", "delete Climbing"
    r"^\s*(?:please\s+)?(?:delete|remove|cancel)\s+(?:the\s+)?[\w][\w\s'-]{0,40}\s*$",
]

CALENDAR_SEARCH = [
    r"\b(?:find|search|look up)\b.+\b(?:event|meeting|appointment|on (?:my )?calendar)\b",
]

DREAM_LOG = [
    r"\b(?:last night'?s?|tonight'?s?)\s+dream\b",
    r"\b(?:log|record|save)\b.+\bdream\b",
    r"\bi dreamed\b",
    r"\bi (?:had a |have a )?dream(?:t)? (?:that|about)\b",
    r"\bdream log\b",
]

DREAM_SEARCH = [
    r"\b(?:show|find|list|search)\b.+\b(?:dreams?|nightmares?)\b",
    r"\ball my nightmares\b",
    r"\bmy nightmares\b",
]

WORK_SALES = [
    r"\bi sold\b",
    r"\bsales?\s*(?:today|of|=|:)?\s*[£$]?\s*\d",
    r"\blog\b.+\bsales?\b",
]

WORK_SET_HOURS = [
    r"\b(?:finished|left|clocked out|ended)\b.+\bwork\b",
    r"\bfinished work at\b",
    r"\bwork(?:ed)? until\b",
]

WORK_STATS = [
    r"\bhow many hours\b.+\bwork",
    r"\bhours (?:have i |did i )?work",
    r"\bwork(?:ing)? hours\b.+\b(?:today|week|month)\b",
    r"\b(?:today'?s|this week'?s|this month'?s)\s+(?:hours|sales)\b",
]


def _extract_json(text: str) -> dict:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\n?", "", text)
        text = re.sub(r"\n?```$", "", text)

    start = text.find("{")
    end = text.rfind("}")
    if start >= 0 and end > start:
        text = text[start : end + 1]

    return json.loads(text)


def _looks_like_idea(message: str) -> bool:
    lower = message.strip().lower()
    return any(re.search(pattern, lower) for pattern in IDEA_TRIGGERS)


def _looks_like_code_request(message: str) -> bool:
    lower = message.strip().lower()
    return any(re.search(pattern, lower) for pattern in CODE_TRIGGERS)


def _matches_any(message: str, patterns: list[str]) -> bool:
    lower = message.strip().lower()
    return any(re.search(pattern, lower) for pattern in patterns)


def _coerce_tool_input(value: Any) -> Optional[str]:
    """Models often emit tool_input as a JSON object; TaskRunner needs a string."""
    if value is None:
        return None
    if isinstance(value, str):
        return value
    try:
        return json.dumps(value)
    except (TypeError, ValueError):
        return str(value)


_DAY_ORDER = ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
_DAY_CODES = {
    "sunday": "SU",
    "sun": "SU",
    "monday": "MO",
    "mon": "MO",
    "tuesday": "TU",
    "tue": "TU",
    "tues": "TU",
    "wednesday": "WE",
    "wed": "WE",
    "thursday": "TH",
    "thu": "TH",
    "thur": "TH",
    "thurs": "TH",
    "friday": "FR",
    "fri": "FR",
    "saturday": "SA",
    "sat": "SA",
}


def _parse_clock_match(hour_s: str, minute_s: str | None, ampm: str | None) -> tuple[int, int]:
    hour = int(hour_s)
    minute = int(minute_s or 0)
    ampm = (ampm or "").lower()
    if ampm == "pm" and hour < 12:
        hour += 12
    elif ampm == "am" and hour == 12:
        hour = 0
    # "16:45pm" stays 16; never add 12 to hours already in 13–23.
    return hour % 24, minute


def _parse_time_range(text: str) -> tuple[tuple[int, int], tuple[int, int]] | None:
    """Parse explicit ranges like 8:45am-16:45pm or 8:45 am to 4:45 pm."""
    m = re.search(
        r"\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\s*(?:-|–|—|to)\s*(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b",
        text,
        flags=re.IGNORECASE,
    )
    if not m:
        return None
    start = _parse_clock_match(m.group(1), m.group(2), m.group(3))
    end = _parse_clock_match(m.group(4), m.group(5), m.group(6))
    return start, end


def _parse_hour_minute(text: str) -> tuple[int, int]:
    """Best-effort single clock time. Ignores duration phrases."""
    tr = _parse_time_range(text)
    if tr:
        return tr[0]
    for m in re.finditer(
        r"\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)\b",
        text,
        flags=re.IGNORECASE,
    ):
        return _parse_clock_match(m.group(1), m.group(2), m.group(3))
    return 9, 0


def _parse_duration_hours(text: str) -> float | None:
    # Prefer H:MM hour before plain "N hours" so "1:30 hour" is 1.5h, not 30h
    # (the digits after ':' would otherwise match as "30 hours").
    m = re.search(r"\b(\d+)\s*:\s*(\d+)\s*hours?\b", text, flags=re.IGNORECASE)
    if m:
        return int(m.group(1)) + int(m.group(2)) / 60.0
    m = re.search(
        r"\b(\d+(?:\.\d+)?)\s*(?:hours?|hrs?)\b",
        text,
        flags=re.IGNORECASE,
    )
    if m:
        return float(m.group(1))
    return None


def _extract_by_days(text: str) -> list[str]:
    lower = text.lower()
    # "mon till fri" / "monday to friday" / "mon-fri"
    range_m = re.search(
        r"\b(mon|monday|tue|tues|tuesday|wed|wednesday|thu|thur|thurs|thursday|fri|friday|sat|saturday|sun|sunday)"
        r"\s*(?:till|until|to|through|-|–|—)\s*"
        r"(mon|monday|tue|tues|tuesday|wed|wednesday|thu|thur|thurs|thursday|fri|friday|sat|saturday|sun|sunday)\b",
        lower,
    )
    if range_m:
        start = _DAY_CODES[range_m.group(1)]
        end = _DAY_CODES[range_m.group(2)]
        i0 = _DAY_ORDER.index(start)
        i1 = _DAY_ORDER.index(end)
        if i0 <= i1:
            return _DAY_ORDER[i0 : i1 + 1]
        return _DAY_ORDER[i0:] + _DAY_ORDER[: i1 + 1]

    found: list[str] = []
    for name, code in _DAY_CODES.items():
        if re.search(rf"\b{re.escape(name)}\b", lower):
            if code not in found:
                found.append(code)
    # Preserve week order
    return [d for d in _DAY_ORDER if d in found]


def _next_weekday(now: datetime, by_day: list[str]) -> datetime:
    code_to_weekday = {"MO": 0, "TU": 1, "WE": 2, "TH": 3, "FR": 4, "SA": 5, "SU": 6}
    targets = [code_to_weekday[c] for c in by_day if c in code_to_weekday]
    if not targets:
        return now + timedelta(days=1)
    for offset in range(0, 8):
        candidate = now + timedelta(days=offset)
        if candidate.weekday() in targets:
            return candidate
    return now + timedelta(days=1)


def _infer_title_category(segment: str) -> tuple[str, str]:
    lower = segment.lower()
    if re.search(r"\bwork\b", lower):
        return "Work", "work"
    if re.search(r"\bclimb", lower):
        return "Climbing", "personal"
    if re.search(r"\bgym\b", lower):
        return "Gym", "personal"
    if re.search(r"\bworkout\b|\btraining\b", lower):
        return "Training", "personal"
    title = re.sub(
        r"^(?:please\s+)?(?:add|create|schedule|book|put)\s+",
        "",
        segment.strip(),
        flags=re.IGNORECASE,
    )
    title = re.sub(
        r"\b(?:every|from|at|on|mon|tue|wed|thu|fri|sat|sun).*$",
        "",
        title,
        flags=re.IGNORECASE,
    ).strip(" .,:-")
    return (title[:60] if title else "New event"), "general"


def _split_schedule_segments(message: str) -> list[str]:
    parts = re.split(
        r"\balong\s*side\b|\balongside\b|\bas well as\b|;|\n",
        message,
        flags=re.IGNORECASE,
    )
    return [p.strip() for p in parts if p.strip()]


def _build_event_payload(segment: str) -> dict[str, Any]:
    lower = segment.lower()
    now = datetime.now().astimezone()
    by_day = _extract_by_days(lower)
    title, category = _infer_title_category(segment)
    time_range = _parse_time_range(lower)
    duration_h = _parse_duration_hours(lower)

    if by_day:
        day = _next_weekday(now, by_day)
    elif "tomorrow" in lower:
        day = now + timedelta(days=1)
    elif "today" in lower or "tonight" in lower:
        day = now
    else:
        day = now + timedelta(days=1)

    if time_range:
        (sh, sm), (eh, em) = time_range
        start = day.replace(hour=sh, minute=sm, second=0, microsecond=0)
        end = day.replace(hour=eh, minute=em, second=0, microsecond=0)
        if end <= start:
            end = end + timedelta(days=1)
    else:
        hour, minute = _parse_hour_minute(lower)
        start = day.replace(hour=hour, minute=minute, second=0, microsecond=0)
        hours = duration_h if duration_h is not None else 1.0
        end = start + timedelta(hours=hours)

    tz = now.tzinfo
    timezone = getattr(tz, "key", None) or "UTC"
    payload: dict[str, Any] = {
        "title": title,
        "start_time": int(start.timestamp() * 1000),
        "end_time": int(end.timestamp() * 1000),
        "category": category,
        "timezone": timezone,
        "reminders": [{"minutes_before": 15, "method": "popup"}],
    }

    if by_day:
        payload["recurrence"] = {
            "frequency": "WEEKLY",
            "interval": 1,
            "by_day": by_day,
        }
    return payload


def _heuristic_create_event_input(message: str) -> str:
    segments = _split_schedule_segments(message)
    events = [_build_event_payload(seg) for seg in segments]
    if len(events) == 1:
        return json.dumps(events[0])
    return json.dumps({"events": events})


def _heuristic_delete_event_input(message: str) -> str:
    """Build delete_event tool_input: clear-all or match by title/query."""
    lower = message.strip().lower()
    if re.search(
        r"\b(?:all|every|everything)\b|\bclear\b.+\b(?:calendar|schedule|events?)\b|\bwipe\b",
        lower,
    ):
        return json.dumps({"all": True})

    title = re.sub(
        r"^(?:please\s+)?(?:delete|remove|cancel)\s+(?:the\s+)?",
        "",
        message.strip(),
        flags=re.IGNORECASE,
    )
    title = re.sub(
        r"\b(?:events?|meetings?|appointments?)\b.*$",
        "",
        title,
        flags=re.IGNORECASE,
    ).strip(" .,:-")
    title = re.sub(
        r"\b(?:from|on)\s+(?:my\s+)?calend(?:a|e)r\b.*$",
        "",
        title,
        flags=re.IGNORECASE,
    ).strip(" .,:-")
    if title:
        return json.dumps({"query": title})
    return json.dumps({"all": True})


def _heuristic_dream_log_input(message: str) -> str:
    text = message.strip()
    body = re.sub(
        r"^(?:please\s+)?(?:log|record|save)\s+(?:this\s+)?(?:dream\s*)(?:[:\-–—]\s*)?",
        "",
        text,
        flags=re.IGNORECASE,
    )
    body = re.sub(
        r"^(?:last night'?s?|tonight'?s?)\s+dream\s*[:\-–—]?\s*",
        "",
        body,
        flags=re.IGNORECASE,
    )
    body = re.sub(
        r"^i dreamed(?: that)?\s+",
        "",
        body,
        flags=re.IGNORECASE,
    )
    body = re.sub(
        r"^i (?:had a |have a )?dream(?:t)? (?:that|about)\s+",
        "",
        body,
        flags=re.IGNORECASE,
    )
    body = body.strip() or text
    tags: list[str] = []
    if re.search(r"\bnightmare\b", text, flags=re.IGNORECASE):
        tags.append("nightmare")
    if re.search(r"\blucid\b", text, flags=re.IGNORECASE):
        tags.append("lucid")
    payload: dict[str, Any] = {"body": body}
    if tags:
        payload["tags"] = tags
    return json.dumps(payload)


def _heuristic_work_sales_input(message: str) -> str:
    m = re.search(
        r"[£$]?\s*(\d+(?:[.,]\d+)?)",
        message.replace(",", ""),
    )
    amount = float(m.group(1)) if m else 0.0
    currency = "GBP"
    if "$" in message and "£" not in message:
        currency = "USD"
    return json.dumps({"amount": amount, "currency": currency})


def _heuristic_work_hours_input(message: str) -> str:
    lower = message.lower()
    m = re.search(
        r"\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)?\b",
        lower,
    )
    if not m:
        return json.dumps({})
    hour, minute = _parse_clock_match(m.group(1), m.group(2), m.group(3))
    # "finished work at 5:15" without am/pm → treat as PM (17:15).
    if m.group(3) is None and hour < 12:
        hour += 12
    return json.dumps({"end_hm": f"{hour:02d}:{minute:02d}"})


def _end_of_local_week_ms(now: Optional[datetime] = None) -> int:
    """Sunday 23:59:59.999 local — used for 'this week' deadlines."""
    now = now or datetime.now().astimezone()
    # Monday=0 … Sunday=6
    days_until_sunday = (6 - now.weekday()) % 7
    end = (now + timedelta(days=days_until_sunday)).replace(
        hour=23, minute=59, second=59, microsecond=999000
    )
    return int(end.timestamp() * 1000)


def _local_day_bounds_ms(day: datetime) -> tuple[int, int]:
    start = day.replace(hour=0, minute=0, second=0, microsecond=0)
    end = start + timedelta(days=1)
    return int(start.timestamp() * 1000), int(end.timestamp() * 1000)


def _heuristic_free_time_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    hours = _parse_duration_hours(lower)
    minutes = int(round((hours if hours is not None else 2.0) * 60))
    # Also catch "120 minutes" / "90 min"
    m = re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower)
    if m and hours is None:
        minutes = int(m.group(1))

    if "tomorrow" in lower:
        day = now + timedelta(days=1)
        start, end = _local_day_bounds_ms(day)
    elif "today" in lower:
        start, end = _local_day_bounds_ms(now)
        start = max(start, int(now.timestamp() * 1000))
    else:
        start = int(now.timestamp() * 1000)
        end = _end_of_local_week_ms(now)

    return json.dumps(
        {
            "duration_minutes": max(minutes, 15),
            "start": start,
            "end": end,
            "limit": 5,
        }
    )


def _heuristic_block_time_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    hours = _parse_duration_hours(lower)
    minutes = int(round((hours if hours is not None else 3.0) * 60))
    title = "Focus"
    if "cod" in lower:
        title = "Coding"
    elif "writ" in lower:
        title = "Writing"
    elif "read" in lower:
        title = "Reading"
    m = re.search(
        r"\bblock\b(?:\s+\d+\s*(?:hours?|hrs?|minutes?|mins?))?\s+(?:for\s+)?(.+)$",
        message.strip(),
        flags=re.IGNORECASE,
    )
    if m:
        raw = re.sub(
            r"\b(?:this week|today|tomorrow|for)\b.*$",
            "",
            m.group(1),
            flags=re.IGNORECASE,
        ).strip(" .,:-")
        if raw and len(raw) < 60:
            title = raw[:1].upper() + raw[1:]
    return json.dumps(
        {
            "title": title,
            "duration_minutes": max(minutes, 30),
            "start": int(now.timestamp() * 1000),
            "end": _end_of_local_week_ms(now),
            "apply": True,
        }
    )


def _heuristic_schedule_task_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    hours = _parse_duration_hours(lower)
    minutes = int(round((hours if hours is not None else 2.0) * 60))
    m = re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower)
    if m and hours is None:
        minutes = int(m.group(1))

    title = re.sub(
        r"^(?:please\s+)?(?:finish|schedule|complete|do)\s+(?:the\s+|my\s+|a\s+)?",
        "",
        message.strip(),
        flags=re.IGNORECASE,
    )
    title = re.sub(
        r"\b(?:this week|today|tomorrow|,?\s*\d+\s*(?:hours?|hrs?|minutes?|mins?)).*$",
        "",
        title,
        flags=re.IGNORECASE,
    ).strip(" .,:-")
    if not title:
        title = "Task"

    if "tomorrow" in lower:
        deadline_day = now + timedelta(days=1)
        deadline = _local_day_bounds_ms(deadline_day)[1] - 1
    elif "today" in lower:
        deadline = _local_day_bounds_ms(now)[1] - 1
    else:
        deadline = _end_of_local_week_ms(now)

    return json.dumps(
        {
            "title": title[:80],
            "duration_minutes": max(minutes, 15),
            "deadline": deadline,
            "priority": "high" if "urgent" in lower or "asap" in lower else "normal",
            "flexibility": "flexible",
            "start": int(now.timestamp() * 1000),
            "end": deadline,
            "apply": True,
        }
    )


def _heuristic_plan_day_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    day = now + timedelta(days=1) if "tomorrow" in lower else now
    day_ms = int(day.replace(hour=12, minute=0, second=0, microsecond=0).timestamp() * 1000)

    # Pull task-like phrases after "plan my day" / commas / and
    rest = re.sub(
        r"^(?:please\s+)?(?:plan|schedule)\s+(?:out\s+)?(?:my\s+|the\s+)?day\s*(?:for\s+)?(?:tomorrow|today)?\s*[,:]?\s*",
        "",
        message.strip(),
        flags=re.IGNORECASE,
    )
    parts = re.split(r"\s*(?:,| and | & |\+)\s*", rest, flags=re.IGNORECASE)
    tasks: list[dict[str, Any]] = []
    for part in parts:
        name = part.strip(" .,:-")
        if not name or len(name) < 2:
            continue
        if re.match(r"^(?:tomorrow|today|please)$", name, flags=re.IGNORECASE):
            continue
        hours = _parse_duration_hours(name)
        minutes = int(round((hours if hours is not None else 1.0) * 60))
        clean = re.sub(
            r"\b\d+\s*(?:hours?|hrs?|minutes?|mins?)\b",
            "",
            name,
            flags=re.IGNORECASE,
        ).strip(" .,:-")
        if not clean:
            continue
        tasks.append(
            {
                "title": clean[:60],
                "duration_minutes": max(minutes, 30),
                "flexibility": "flexible",
                "priority": "normal",
            }
        )

    return json.dumps(
        {
            "day": day_ms,
            "tasks": tasks,
            "include_breaks": True,
            "apply": True,
        }
    )


def _heuristic_capacity_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    day = now + timedelta(days=1) if "tomorrow" in lower else now
    day_ms = int(day.replace(hour=12, minute=0, second=0, microsecond=0).timestamp() * 1000)
    return json.dumps({"day": day_ms})


def _extract_idea_content(message: str) -> str:
    text = message.strip()
    lead_ins = [
        r"^i have an idea(?: for)?\s*[:—–-]?\s*",
        r"^idea for\s*[:—–-]?\s*",
        r"^what if we\s*[:—–-]?\s*",
        r"^what if i\s*[:—–-]?\s*",
        r"^note to self\s*[:—–-]?\s*",
        r"^thinking about\s*[:—–-]?\s*",
        r"^thought about\s*[:—–-]?\s*",
        r"^maybe we could\s*[:—–-]?\s*",
        r"^maybe i could\s*[:—–-]?\s*",
        r"^i('ve| have) been thinking(?: about)?\s*[:—–-]?\s*",
        r"^spark:\s*",
        r"^spark\s+",
    ]
    for pattern in lead_ins:
        stripped = re.sub(pattern, "", text, flags=re.IGNORECASE).strip()
        if stripped and stripped != text:
            return stripped
    return text


def _heuristic_plan(message: str) -> PlanResponse:
    lower = message.strip().lower()
    if lower.startswith("/handover") or lower in ("handover", "create handover"):
        return PlanResponse(
            intent="tool_use",
            tool="memory.handover",
            tool_input="{}",
            reasoning="User requested a conversation handover.",
            response=None,
        )
    if lower.startswith("/maintain") or "maintain memory" in lower:
        return PlanResponse(
            intent="tool_use",
            tool="memory.maintain",
            tool_input="{}",
            reasoning="User requested memory maintenance.",
            response=None,
        )
    if lower.startswith("echo "):
        tool_input = message.strip()[5:].strip()
        return PlanResponse(
            intent="tool_use",
            tool="echo",
            tool_input=tool_input or message.strip(),
            reasoning="User requested echo tool via prefix.",
            response=None,
        )
    if lower == "echo":
        return PlanResponse(
            intent="tool_use",
            tool="echo",
            tool_input="",
            reasoning="User requested echo with no input.",
            response=None,
        )
    if _looks_like_idea(message):
        content = _extract_idea_content(message)
        tags = _infer_spark_tags(content.lower())
        return PlanResponse(
            intent="tool_use",
            tool="save_spark",
            tool_input=json.dumps({"content": content, "tags": tags}),
            reasoning="User shared an idea in natural language.",
            response=None,
        )
    if _looks_like_code_request(message):
        return PlanResponse(
            intent="tool_use",
            tool="coder.run",
            tool_input=json.dumps({"prompt": message, "focus": "focused"}),
            reasoning="User asked for code changes (heuristic fallback, MLX unavailable).",
            response=None,
        )
    # Scheduling intelligence — before agenda queries so "free tomorrow" ≠ get_tomorrow.
    if _matches_any(message, CALENDAR_FREE_TIME):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.find_free_time",
            tool_input=_heuristic_free_time_input(message),
            reasoning="User asked for free/available time.",
            response=None,
        )
    if _matches_any(message, CALENDAR_PLAN_DAY):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.plan_day",
            tool_input=_heuristic_plan_day_input(message),
            reasoning="User asked to plan their day.",
            response=None,
        )
    if _matches_any(message, CALENDAR_BLOCK_TIME):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.block_time",
            tool_input=_heuristic_block_time_input(message),
            reasoning="User asked to block focus time.",
            response=None,
        )
    if _matches_any(message, CALENDAR_SCHEDULE_TASK):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.schedule_task",
            tool_input=_heuristic_schedule_task_input(message),
            reasoning="User asked to schedule a task into free time.",
            response=None,
        )
    if _matches_any(message, CALENDAR_CAPACITY):
        tool = (
            "calendar.day_summary"
            if re.search(r"\bsummar", message, flags=re.IGNORECASE)
            else "calendar.get_capacity"
        )
        return PlanResponse(
            intent="tool_use",
            tool=tool,
            tool_input=_heuristic_capacity_input(message),
            reasoning="User asked about daily capacity or summary.",
            response=None,
        )
    if _matches_any(message, CALENDAR_QUERY_TODAY):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.get_today",
            tool_input="{}",
            reasoning="User asked about today's calendar.",
            response=None,
        )
    if _matches_any(message, CALENDAR_QUERY_TOMORROW):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.get_tomorrow",
            tool_input="{}",
            reasoning="User asked about tomorrow's calendar.",
            response=None,
        )
    if _matches_any(message, CALENDAR_QUERY_WEEK):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.get_this_week",
            tool_input="{}",
            reasoning="User asked about this week's calendar.",
            response=None,
        )
    if _matches_any(message, CALENDAR_CREATE):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.create_event",
            tool_input=_heuristic_create_event_input(message),
            reasoning="User asked to schedule a calendar event.",
            response=None,
        )
    if _matches_any(message, CALENDAR_SEARCH):
        q = re.sub(
            r"^(?:please\s+)?(?:find|search|look up)\s+",
            "",
            message.strip(),
            flags=re.IGNORECASE,
        )
        return PlanResponse(
            intent="tool_use",
            tool="calendar.search_events",
            tool_input=json.dumps({"query": q.strip() or message.strip()}),
            reasoning="User asked to search the calendar.",
            response=None,
        )
    if _matches_any(message, CALENDAR_DELETE):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.delete_event",
            tool_input=_heuristic_delete_event_input(message),
            reasoning="User asked to remove calendar event(s).",
            response=None,
        )
    if _matches_any(message, DREAM_SEARCH):
        q = "nightmare" if "nightmare" in lower else message.strip()
        if "nightmare" in lower:
            q = "nightmare"
        return PlanResponse(
            intent="tool_use",
            tool="dream.search",
            tool_input=json.dumps({"query": q}),
            reasoning="User asked to search dream log.",
            response=None,
        )
    if _matches_any(message, DREAM_LOG):
        return PlanResponse(
            intent="tool_use",
            tool="dream.log",
            tool_input=_heuristic_dream_log_input(message),
            reasoning="User asked to log a dream.",
            response=None,
        )
    if _matches_any(message, WORK_SALES):
        return PlanResponse(
            intent="tool_use",
            tool="work.log_sales",
            tool_input=_heuristic_work_sales_input(message),
            reasoning="User logged sales.",
            response=None,
        )
    if _matches_any(message, WORK_SET_HOURS):
        return PlanResponse(
            intent="tool_use",
            tool="work.set_hours",
            tool_input=_heuristic_work_hours_input(message),
            reasoning="User adjusted work hours.",
            response=None,
        )
    if _matches_any(message, WORK_STATS):
        return PlanResponse(
            intent="tool_use",
            tool="work.get_stats",
            tool_input="{}",
            reasoning="User asked about work hours/sales stats.",
            response=None,
        )
    fallback_response = _heuristic_chat_response(message)
    return PlanResponse(
        intent="chat",
        tool=None,
        tool_input=None,
        reasoning="General conversation fallback (MLX unavailable).",
        response=fallback_response,
    )


def _infer_spark_tags(text: str) -> list[str]:
    tags: list[str] = []
    if any(w in text for w in ("van", "camper", "roof rack", "solar panel")):
        tags.append("the_van")
    if any(w in text for w in ("land", "garden", "fence", "trees", "property")):
        tags.append("the_land")
    if any(w in text for w in ("trip", "travel", "road trip", "scotland", "journey")):
        tags.append("travelling")
    if any(w in text for w in ("project", "app", "build", "code")):
        tags.append("projects")
    if not tags:
        tags.append("general_life")
    return tags


def _heuristic_chat_response(message: str) -> str:
    text = message.strip()
    if not text:
        return "I'm here and listening. What would you like to work on?"
    lowered = text.lower()
    if "?" in text:
        if any(greet in lowered for greet in ("how are you", "how's it going", "hows it going")):
            return "Doing well and ready to help. What would you like to focus on?"
        return f"I got your question: \"{text}\". I can help sketch an answer while MLX reconnects."
    return f"Noted: \"{text}\". I can help you structure or refine this while full chat is reconnecting."


def normalize_plan(plan: PlanResponse) -> PlanResponse:
    """Repair common planner mistakes so tools actually execute.

    Models often set intent to the tool name (e.g. "calendar.create_event")
    or leave intent as "chat" while setting tool to an alias like
    "calendar.add_event". Buddy only routes plans; this repair stays in the Brain.

    Legacy intent "code" is normalised to tool_use + coder.run (single pipeline).
    """
    intent = (plan.intent or "chat").strip()
    tool = plan.tool.strip() if isinstance(plan.tool, str) else plan.tool
    tool_input = plan.tool_input
    mode_hint = plan.mode_hint

    # Legacy code intent → Core coder.run tool.
    if intent == "code":
        intent = "tool_use"
        tool = "coder.run"
        if not tool_input or tool_input in ("{}", "null"):
            payload = {"prompt": "", "focus": mode_hint or "focused"}
            tool_input = json.dumps(payload)

    # Intent mistakenly set to a tool name.
    if tool is None and ("." in intent or intent.startswith("calendar") or intent.startswith("coder") or intent.startswith("memory")):
        tool = intent
        intent = "tool_use"

    if isinstance(tool, str) and tool:
        tool = TOOL_ALIASES.get(tool, tool)
        if intent != "tool_use":
            intent = "tool_use"

    if tool and tool_input is None:
        tool_input = "{}"

    plan.intent = intent
    plan.tool = tool
    plan.tool_input = tool_input
    return apply_respond_mode(plan)


def parse_plan(raw: str, message: str) -> PlanResponse:
    try:
        data = _extract_json(raw)
        pref = data.get("preference_detected")
        dec = data.get("decision_detected")
        tool = data.get("tool")
        tool_input = _coerce_tool_input(data.get("tool_input"))
        # Tools that allow empty input still need a JSON object string.
        if tool and tool_input is None:
            tool_input = "{}"
        plan = PlanResponse(
            intent=data.get("intent", "chat"),
            tool=tool,
            tool_input=tool_input,
            reasoning=data.get("reasoning", ""),
            response=data.get("response"),
            task_state=data.get("task_state"),
            mode_hint=data.get("mode_hint"),
            preference_detected=PreferenceDetected(**pref) if pref else None,
            decision_detected=DecisionDetected(**dec) if dec else None,
        )
        return normalize_plan(plan)
    except (json.JSONDecodeError, KeyError, TypeError, ValidationError, ValueError):
        return normalize_plan(_heuristic_plan(message))


def parse_extraction(kind: str, raw: str) -> dict:
    try:
        return _extract_json(raw)
    except (json.JSONDecodeError, TypeError):
        if kind == "handover":
            return {"summary": raw.strip() or "No handover content generated."}
        if kind == "reflection":
            return {
                "attempted": "Unknown task",
                "successful": False,
                "improvements": "",
                "lessons": raw.strip(),
            }
        if kind == "project":
            return {"section": "general", "content": raw.strip()}
        if kind == "conversation_archive":
            return {
                "summary": raw.strip() or "Archived conversation.",
                "topics": [],
                "key_facts": [],
                "decisions": [],
            }
        if kind == "spark_archive":
            return {
                "summary": raw.strip() or "Deleted spark.",
                "topics": [],
                "key_facts": [],
            }
        return {"raw": raw.strip()}
