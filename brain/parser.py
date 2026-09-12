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
    # Agent loop: ask | tool | finish (null → derived from intent).
    next: Optional[str] = None


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
_PASSTHROUGH_PREFIXES = ("calendar.", "dream.", "work.", "docs.")
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
    # Soft idea language goes through MLX — only high-precision spark phrases skip it.
    if plan.tool == "save_spark" and not _looks_like_idea_fast(message):
        return None
    # Never invent a fixed create_event for soft slot booking — schedule into free time.
    if plan.tool in ("calendar.create_event", "calendar.pin") and _is_soft_slot_booking(
        message
    ):
        plan = PlanResponse(
            intent="tool_use",
            tool="calendar.organize",
            tool_input=_heuristic_organize_input(message),
            reasoning="Soft booking without a fixed clock — organize into free time.",
            response=None,
        )
        plan = normalize_plan(plan)
    # Chat fallback from heuristics always says MLX unavailable — not a fast path.
    if plan.reasoning and "MLX unavailable" in plan.reasoning and plan.tool is None:
        return None
    return apply_respond_mode(plan)

# Broad idea language — used when MLX is unavailable (full heuristic fallback).
IDEA_TRIGGERS = [
    r"\bi have an idea\b",
    r"\bi got an idea\b",
    r"\bhere'?s an idea\b",
    r"\bidea for\b",
    r"\bidea:\s*",
    r"\bwhat if we\b",
    r"\bwhat if i\b",
    r"\bwhat if\b",
    r"\bthought about\b",
    r"\bthinking about\b",
    r"\bi was thinking\b",
    r"\bjust thought of\b",
    r"\brandom thought\b",
    r"\bbrain dump\b",
    r"\bjot this down\b",
    r"\bsave this (?:idea|thought|note)\b",
    r"\bkeep in mind\b",
    r"\bworth trying\b",
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

# High-precision only — safe to skip MLX on the fast path.
IDEA_TRIGGERS_FAST = [
    r"\bbrain dump\b",
    r"\bjot this down\b",
    r"\bsave this (?:idea|thought|note)\b",
    r"\bnote to self\b",
    r"\bspark:\s*",
    r"\bspark\s+\w",
]

# Heuristic-only triggers for when MLX is unreachable; the real planner
# decides "code" intent from full context, not just these phrases.
CODE_TRIGGERS = [
    r"\bcan you code\b",
    r"\bwrite code\b",
    r"\bhelp me code\b",
    r"\bwrite (?:a|me a|some) (?:script|function|program|app)\b",
    r"\bcreate (?:a|an) (?:script|function)\b",
    r"\badd (?:a|an) (?:function|feature)\b",
    r"\bimplement\b",
    r"\bfix (?:this|the) bug\b",
    r"\bfix (?:a|this|the) (?:error|issue)\b",
    r"\bdebug (?:this|my)\b",
    r"\brefactor\b",
    r"\bbuild (?:a|me a) (?:script|app|tool)\b",
]

CALENDAR_QUERY_TODAY = [
    r"\bwhat(?:'s| is) on (?:my )?(?:calendar )?today\b",
    r"\bwhat(?:'s| is) happening today\b",
    r"\btoday'?s (?:schedule|agenda|events|calendar)\b",
    r"\bagenda for today\b",
    r"\bmy (?:day|schedule) today\b",
    r"\blist (?:my )?events today\b",
    r"\b(?:anything|got anything) (?:on (?:my )?calendar )?today\b",
    r"\bshow (?:me )?(?:my )?(?:calendar |schedule |agenda )?today\b",
    r"\bdo i have anything today\b",
]

CALENDAR_QUERY_WORK = [
    r"\bwhen am i working\b",
    r"\bwhen do i work\b",
    r"\bwhat time do i work\b",
    r"\bwhat are my work hours\b",
    r"\bwhen do i finish work\b",
    r"\bwhen does work end\b",
    r"\bwhen am i at work\b",
]

CALENDAR_QUERY_TOMORROW = [
    r"\bwhat(?:'s| is) on (?:my )?(?:calendar )?tomorrow\b",
    r"\bwhat(?:'s| is) happening tomorrow\b",
    r"\btomorrow'?s (?:schedule|agenda|events|calendar)\b",
    r"\bagenda for tomorrow\b",
    r"\bmy (?:day|schedule) tomorrow\b",
    r"\blist (?:my )?events tomorrow\b",
    r"\b(?:anything|got anything) (?:on (?:my )?calendar )?tomorrow\b",
    r"\bshow (?:me )?(?:my )?(?:calendar |schedule |agenda )?tomorrow\b",
    r"\bdo i have anything tomorrow\b",
]

CALENDAR_QUERY_WEEK = [
    r"\bwhat(?:'s| is) (?:on )?(?:my )?(?:calendar )?this week\b",
    r"\bwhat(?:'s| is) happening this week\b",
    r"\bthis week'?s (?:schedule|agenda|events|calendar)\b",
    r"\bagenda for (?:this )?week\b",
    r"\bmy (?:schedule|calendar) this week\b",
    r"\blist (?:my )?events this week\b",
    r"\b(?:anything|got anything) (?:on (?:my )?calendar )?this week\b",
    r"\bshow (?:me )?(?:my )?(?:calendar |schedule |agenda )?this week\b",
    r"\bdo i have anything this week\b",
]

CALENDAR_LOOK_CUES = [
    r"\bwhat(?:'s|s| is) on\b",
    r"\bwhat(?:'s|s| is) happening\b",
    r"\bwhat(?:'s|s| is) planned\b",
    r"\bon my calendar\b",
    r"\bmy (?:calendar|schedule|agenda|plans)\b",
    r"\banything\b",
    r"\bshow me\b",
    r"\bdo i have\b",
    r"\bagenda\b",
    r"\bevents?\b",
    r"\bplans\b",
    r"\bgive me\b.+\b(?:week|calendar|agenda|schedule|slots?|free)\b",
    r"\b(?:all|whole|entire) week\b",
]

CALENDAR_FREE_TIME = [
    r"\bwhen am i free\b",
    r"\bwhen (?:do i|can i) have free\b",
    r"\bam i free\b",
    r"\bfind (?:me )?(?:\d+\s*(?:hours?|hrs?|minutes?|mins?)|free time|a (?:free )?slot|an? opening)\b",
    r"\b(?:am i|do i have|got) (?:any )?free\b",
    r"\bgot any free time\b",
    r"\bwhat(?:'s| is) (?:my )?availability\b",
    r"\bopen slots?\b",
    r"\bfree slots?\b",
    r"\bfree windows?\b",
    r"\bgaps? in (?:my )?schedule\b",
    r"\bavailable (?:tomorrow|today|this week)\b",
    r"\bfree (?:tomorrow|today|this week)\b",
]

# Imperative "block …" / focus phrasing only — not noun phrases like "research block for 2h".
CALENDAR_BLOCK_TIME = [
    r"^(?:please\s+)?block\s+(?:off|out|time)\b",
    r"^(?:please\s+)?block\s+\d+\s*(?:hours?|hrs?|minutes?|mins?)\b",
    r"^(?:please\s+)?block\s+for\b",
    r"\b(?:can you|could you|please)\s+block\s+(?:off|out|time|\d+|for)\b",
    r"^(?:please\s+)?(?:hold|reserve|set aside)\s+(?:\d+\s*(?:hours?|hrs?|minutes?|mins?)|time)\b",
    r"\b(?:focus|deep work)\b.+\b(?:hours?|block)\b",
    r"\bfocus\s+block\b",
]

_CAL_ACTIVITY = (
    r"climbing|climb|gym|workout|training|yoga|tennis|lunch|"
    r"study|studying|dentist|doctor|interview|standup|stand-?up|"
    r"swim|swimming|cycle|cycling|run|running|walk|walking|"
    r"pilates|cook|cooking|bath"
)

_COUNT_WORDS = {
    "a": 1,
    "an": 1,
    "one": 1,
    "once": 1,
    "two": 2,
    "twice": 2,
    "couple": 2,
    "three": 3,
    "thrice": 3,
    "four": 4,
    "five": 5,
    "six": 6,
    "seven": 7,
    "eight": 8,
    "nine": 9,
    "ten": 10,
}

_ACTIVITY_ALIASES = {
    "climb": "Climbing",
    "climbing": "Climbing",
    "gym": "Gym",
    "workout": "Training",
    "training": "Training",
    "yoga": "Yoga",
    "tennis": "Tennis",
    "lunch": "Lunch",
    "study": "Study",
    "studying": "Study",
    "dentist": "Dentist",
    "doctor": "Doctor",
    "interview": "Interview",
    "standup": "Standup",
    "stand-up": "Standup",
    "swim": "Swim",
    "swimming": "Swim",
    "cycle": "Cycling",
    "cycling": "Cycling",
    "run": "Run",
    "running": "Run",
    "walk": "Walk",
    "walking": "Walk",
    "pilates": "Pilates",
    "cook": "Cooking",
    "cooking": "Cooking",
    "bath": "Bath",
    "code": "Coding",
    "coding": "Coding",
    "spark": "Spark",
    "sparks": "Spark",
}

_PURPOSE_FILLERS = frozenset(
    {
        "include",
        "plan",
        "finish",
        "make",
        "get",
        "book",
        "do",
        "go",
        "spend",
        "have",
        "take",
        "fit",
        "schedule",
        "want",
        "need",
        "try",
        "add",
        "put",
        "set",
        "keep",
        "use",
        "find",
        "block",
        "hold",
        "reserve",
        "organise",
        "organize",
        "help",
        "start",
        "work",
    }
)

_JUNK_TITLES = frozenset(
    {
        "session",
        "sessions",
        "include",
        "want",
        "week",
        "time",
        "times",
        "plan",
        "after",
        "work",
        "my",
        "the",
        "to",
        "for",
        "next",
        "this",
        "today",
        "tomorrow",
        "weekend",
        "hour",
        "hours",
        "min",
        "mins",
        "minute",
        "minutes",
        "and",
        "or",
        "a",
        "an",
        "some",
        "it",
        "them",
        "these",
        "that",
        "with",
    }
)

_ACTIVITY_ALT = "|".join(
    sorted((_ACTIVITY_ALIASES.keys()), key=len, reverse=True)
)
_CAL_WHEN = (
    r"hour|am|pm|monday|tuesday|wednesday|thursday|friday|saturday|sunday|"
    r"mon|tue|wed|thu|fri|sat|sun|today|tomorrow|midday|noon"
)

CALENDAR_SCHEDULE_TASK = [
    r"\bfinish\b.+\b(?:this week|today|tomorrow)\b",
    r"\bneed to finish\b.+\b(?:by|this week|today|tomorrow)\b",
    r"\bget\b.+\bdone\b.+\b(?:this week|today|tomorrow)\b",
    r"\bfit\b.+\binto (?:my )?week\b",
    r"\bplan (?:out )?(?:my )?week\b",
    r"\bschedule (?:out )?(?:my )?week\b",
    r"\bfind time for\b",
    r"\bschedule (?:the |a |my )?(?:task|report|project)\b",
    r"\bauto[- ]?schedule\b",
    r"\b\d+\s+times?\s+(?:(?:a|per)\s+week|(?:this|next)\s+week|(?:this|next)\s+month)\b",
    r"\b(?:once|twice|thrice)\s+(?:(?:a|per)|(?:this|next))\s+week\b",
    r"\bi want to (?:go|do|get to)\b.+\b\d+\s+times?\b",
    r"\bi want to (?:go|do|get to|make time for)\b.+\b(?:this|next)\s+week\b",
    r"\bmake time for\b",
    r"\bfit in\b.+\b(?:this week|today|tomorrow|\d+\s+times?)\b",
    r"\bschedule\b.+\b\d+\s+times?\b",
    # Soft NL: book time / book an activity without a fixed clock time.
    r"\bbook (?:some |a )?time\b",
    rf"\bbook\b.+\bfor\b.+\b(?:{_CAL_ACTIVITY})\b",
    r"\b(?:want to|wanna|need to)\s+book\b",
    r"\bi want to book\b",
    rf"\bbook\b.+\b(?:{_CAL_ACTIVITY})\b",
    # Multi-session week packing ("3 30min study sessions, 2 climbing…").
    r"\b\d+\s+(?:\d+\s*(?:min(?:ute)?s?|hours?|hrs?|hr)|(?:\d+:\d+)\s*(?:hours?|hrs?|hr)?)\s+\w[\w\s]{0,40}?\s+sessions?\b",
    r"\b\d+\s+\w[\w\s]{0,40}?\s+sessions?\s+for\b",
]

CALENDAR_PLAN_DAY = [
    r"\bplan my day\b",
    r"\bplan (?:out )?(?:my |the )?day\b",
    r"\bschedule my day\b",
    r"\borgani[sz]e (?:my )?(?:day|tomorrow)\b",
    r"\bmap out (?:my )?day\b",
    r"\bfill (?:my )?day\b",
    r"\bplan (?:out )?(?:my )?tomorrow(?:'?s)?\s+day\b",
    r"\bplan (?:out )?(?:my )?tomorrow\s*$",
    r"\bi want to\b.+\bon (?:sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b",
    r"\bon (?:sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b.+\b(?:and|,|gym|spark|climb|tennis|bath|cook|yoga|run)\b",
    r"\b(?:this|next)\s+(?:sunday|saturday|monday|tuesday|wednesday|thursday|friday)\b.+\b(?:and|,|gym|spark)\b",
    r"\bdo\b.+\bsparks?\b.+\bon (?:sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b",
]

CALENDAR_CAPACITY = [
    r"\b(?:what(?:'s| is)|how(?:'s| is)) (?:my )?(?:capacity|workload)\b",
    r"\bcapacity (?:today|tomorrow)\b",
    r"\bworkload (?:today|tomorrow)\b",
    r"\bhow(?:'s| is) (?:my )?day looking\b",
    r"\bhow packed is\b",
    r"\bhow busy (?:am i|is my day)\b",
    r"\bam i overloaded\b",
    r"\bday overview\b",
    r"\bdaily (?:summary|capacity)\b",
    r"\bsummar(?:y|ise|ize) (?:my )?(?:day|today)\b",
]

CALENDAR_CREATE = [
    r"\b(?:add|create|schedule|book|put|make|set up|pencil in|slot in|throw on)\b.+\b(?:event|meeting|appointment|call|reminder)\b",
    r"\b(?:add|create|schedule|book|put|make)\b.+\b(?:lunch|break)\b",
    r"\blunch\s+break\b",
    r"\bbreak\s+(?:around|at|for|around)\b",
    r"\blunch\b.+\b(?:midday|noon|today|tomorrow|at)\b",
    r"\bnew (?:event|meeting|appointment|call|reminder)\b",
    r"\bevent\s+(?:called|named)\b",
    r"\bmeeting with\b",
    r"\bcall with\b.+\bat\b",
    r"\bappointment for\b",
    r"\b(?:add|put|schedule)\b.+\b(?:on|to)\b.+\bcalend(?:a|e)r\b",
    r"\bput\b.+\bin (?:my )?(?:calend(?:a|e)r|diary)\b",
    r"\badd (?:this|that|it) to (?:my )?(?:calend(?:a|e)r)\b",
    r"\bremind me (?:to|about)\b",
    r"\bevery\s+(?:monday|tuesday|wednesday|thursday|friday|saturday|sunday)\b",
    # Fixed clock only for activity+when create — soft day-only booking is schedule_task.
    rf"\b(?:{_CAL_ACTIVITY})\b.+\b(?:\d{{1,2}}(?::\d{{2}})?\s*(?:am|pm)|noon|midday)\b",
    rf"\bi have (?:{_CAL_ACTIVITY})\b.+\b(?:\d{{1,2}}(?::\d{{2}})?\s*(?:am|pm)|noon|midday)\b",
    r"\b\d{1,2}:\d{2}\s*(?:am|pm)?\s*(?:-|to)\s*\d{1,2}:\d{2}",
    r"\bmon(?:day)?\s*(?:till|to|through|-)\s*fri",
]

TOOL_ALIASES = {
    "calendar.add_event": "calendar.pin",
    "calendar.schedule_event": "calendar.pin",
    "calendar.add": "calendar.pin",
    "calendar.schedule": "calendar.organize",
    "calendar.create": "calendar.pin",
    "calendar.create_event": "calendar.pin",
    "calendar.schedule_task": "calendar.organize",
    "calendar.plan_day": "calendar.organize",
    "calendar.block_time": "calendar.organize",
    "calendar.find_free_time": "calendar.look",
    "calendar.get_today": "calendar.look",
    "calendar.get_tomorrow": "calendar.look",
    "calendar.get_this_week": "calendar.look",
    "calendar.search_events": "calendar.look",
    "calendar.get_capacity": "calendar.look",
    "calendar.day_summary": "calendar.look",
    "calendar.delete_event": "calendar.pin",
    "calendar.update_event": "calendar.pin",
}

CALENDAR_DELETE = [
    r"\b(?:delete|remove|cancel|scratch|drop|unschedule)\b.+\b(?:events?|meetings?|appointments?)\b",
    r"\b(?:delete|remove|cancel|scratch|drop|unschedule)\b.+\b(?:from )?(?:my )?calend(?:a|e)r\b",
    r"\bget rid of\b.+\b(?:events?|meetings?|appointments?|(?:my )?calend(?:a|e)r)\b",
    r"\btake (?:it |them )?off (?:my )?calend(?:a|e)r\b",
    r"\b(?:clear|wipe|empty|scrub)\b.+\b(?:calendar|schedule|events?)\b",
    r"\bcancel my\b",
    r"\bremove all\b",
    r"\bclear (?:my )?calendar\b",
    # Short title deletes: "remove gym", "delete Climbing"
    r"^\s*(?:please\s+)?(?:delete|remove|cancel|scratch|drop|unschedule)\s+(?:the\s+)?[\w][\w\s'-]{0,40}\s*$",
]

CALENDAR_SEARCH = [
    r"\b(?:find|search|look up|look for)\b.+\b(?:event|meeting|appointment|on (?:my )?calendar|in (?:my )?calendar)\b",
    r"\bwhere is\b.+\bon (?:my )?calend(?:a|e)r\b",
    r"\bwhen is (?:my )?.+\b(?:event|meeting|appointment|on (?:my )?calend(?:a|e)r)\b",
    r"\bdo i have (?:a|an|any)\b.+\b(?:event|meeting|appointment|on (?:my )?calend(?:a|e)r)\b",
]

DREAM_LOG = [
    r"\b(?:last night'?s?|tonight'?s?)\s+dream\b",
    r"\b(?:log|record|save|write down)\b.+\bdream\b",
    r"\bwrite down (?:this )?dream\b",
    r"\bi dreamed\b",
    r"\bi dreamt\b",
    r"\bi (?:had a |have a )?dream(?:t)? (?:that|about)\b",
    r"\bi had a dream\b",
    r"\bnightmare (?:last night|about)\b",
    r"\bdream(?:t)? (?:that|about)\b",
    r"\bdream log\b",
]

DREAM_SEARCH = [
    r"\b(?:show|find|list|search|recall)\b.+\b(?:dreams?|nightmares?)\b",
    r"\bdream journal\b",
    r"\bpast dreams\b",
    r"\brecall (?:my )?dreams\b",
    r"\ball my nightmares\b",
    r"\bmy nightmares\b",
]

WORK_SALES = [
    r"\bi sold\b",
    r"\bmade a sale\b",
    r"\bclosed\b.+\bfor\s*[£$]\s*\d",
    r"\brevenue today\b",
    r"\bsales?\s*(?:today|of|=|:)?\s*[£$]?\s*\d",
    r"\blog\b.+\bsales?\b",
]

WORK_SET_HOURS = [
    r"\b(?:finished|left|clocked out|clocked off|ended|stopped)\b.+\bwork\b",
    r"\bfinished work at\b",
    r"\bstopped work at\b",
    r"\bknocked off at\b",
    r"\bdone for the day at\b",
    r"\bwork(?:ed)? until\b",
]

WORK_STATS = [
    r"\bhow many hours\b.+\bwork",
    r"\bhours (?:have i |did i )?work",
    r"\btotal hours\b",
    r"\bwork(?:ing)? hours\b.+\b(?:today|week|month)\b",
    r"\b(?:today'?s|this week'?s|this month'?s)\s+(?:hours|sales)\b",
    r"\bsales this (?:week|month)\b",
    r"\bhow much did i sell\b",
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


def _looks_like_idea_fast(message: str) -> bool:
    lower = message.strip().lower()
    return any(re.search(pattern, lower) for pattern in IDEA_TRIGGERS_FAST)


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


def _has_explicit_clock(text: str) -> bool:
    """True if the segment names a clock time or explicit range (not just a duration)."""
    if _parse_time_range(text):
        return True
    if re.search(r"\b\d{1,2}(?::\d{2})?\s*(?:am|pm)\b", text, flags=re.IGNORECASE):
        return True
    if re.search(r"\b\d{1,2}:\d{2}\b", text):
        return True
    if re.search(r"\b(?:noon|midday)\b", text, flags=re.IGNORECASE):
        return True
    return False


def _is_soft_slot_booking(message: str) -> bool:
    """True when the user wants a slot found, not a fixed-clock appointment."""
    if _has_explicit_clock(message):
        return False
    if re.search(
        r"\b(?:meeting|appointment|event|call|reminder|lunch|break)\b",
        message,
        flags=re.IGNORECASE,
    ):
        return False
    if _matches_any(message, CALENDAR_SCHEDULE_TASK):
        return True
    if re.search(r"\bbook\b", message, flags=re.IGNORECASE):
        return True
    if re.search(rf"\b(?:{_CAL_ACTIVITY})\b", message, flags=re.IGNORECASE) and re.search(
        r"\b(?:today|tomorrow|this week|next week)\b",
        message,
        flags=re.IGNORECASE,
    ):
        return True
    return False


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
    # Bare HH:MM (e.g. "at 11:00") — treat as 24h local clock.
    for m in re.finditer(r"\b(\d{1,2}):(\d{2})\b", text):
        hour, minute = int(m.group(1)), int(m.group(2))
        if 0 <= hour <= 23 and 0 <= minute <= 59:
            return hour, minute
    if re.search(r"\b(?:midday|noon)\b", text, flags=re.IGNORECASE):
        return 12, 0
    return 9, 0


def _is_lunch_or_break(text: str) -> bool:
    return bool(
        re.search(r"\blunch\b", text, flags=re.IGNORECASE)
        or re.search(r"\b(?:lunch\s+)?break\b", text, flags=re.IGNORECASE)
    )

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

    category = "general"
    cat_m = re.search(
        r"\b(personal|work|birthdays|holidays|general)\s+"
        r"(?:event|meeting|appointment|call|reminder)\b",
        lower,
    )
    if cat_m:
        category = cat_m.group(1)
    elif re.search(r"\bpersonal\b", lower):
        category = "personal"
    elif re.search(r"\bwork\b", lower) and not re.search(
        r"\b(?:to|for)\s+(?:code|coding|climb|gym)\b", lower
    ):
        category = "work"

    activity_map = {
        "code": "Coding",
        "coding": "Coding",
        "climb": "Climbing",
        "climbing": "Climbing",
        "gym": "Gym",
        "workout": "Training",
        "training": "Training",
        "lunch": "Lunch",
    }

    if re.search(r"\blunch\b", lower):
        return "Lunch", "personal" if category == "general" else category
    if re.search(r"\bbreak\b", lower) and not re.search(
        r"\b(?:block|research)\b", lower
    ):
        return "Break", "personal" if category == "general" else category

    # "to code" / "for coding" — skip glue ("to include", "to finish").
    purpose = re.search(
        r"\b(?:to|for)\s+(?!"
        r"(?:\d+|a|an|the|me|my|book|"
        r"monday|tuesday|wednesday|thursday|friday|saturday|sunday|"
        r"tomorrow|today|tonight|hours?|hrs?|minutes?|mins?)\b)"
        r"(?:go\s+|do\s+)?([a-z]+)\b",
        lower,
    )
    if purpose:
        raw = purpose.group(1).strip()
        if raw not in _PURPOSE_FILLERS:
            title = activity_map.get(
                raw, _ACTIVITY_ALIASES.get(raw, raw[:1].upper() + raw[1:] if raw else "New event")
            )
            if category == "general" and (raw in activity_map or raw in _ACTIVITY_ALIASES):
                category = "personal"
            return title[:60], category

    if re.search(r"\bclimb", lower):
        return "Climbing", "personal" if category == "general" else category
    if re.search(r"\bgym\b", lower):
        return "Gym", "personal" if category == "general" else category
    if re.search(r"\bworkout\b|\btraining\b", lower):
        return "Training", "personal" if category == "general" else category
    if re.search(r"\bcod(?:e|ing)\b", lower):
        return "Coding", "personal" if category == "general" else category
    if category == "work" and re.search(r"\bwork\b", lower):
        return "Work", "work"

    called = re.search(
        r"\b(?:called|named)\s+(.+?)(?=\s+(?:for|at|on|from|tomorrow|today|tonight|every|followed|then|\d)|$)",
        segment,
        flags=re.IGNORECASE,
    )
    if called:
        title = called.group(1).strip(" .,:-")
        if title:
            return title[:60], category

    noun_m = re.search(
        r"\b(meeting|appointment|call|reminder|event)\b", lower
    )
    title = re.sub(
        r"^(?:please\s+)?(?:add|create|schedule|book|put|make|set up|pencil in|i have)\s+",
        "",
        segment.strip(),
        flags=re.IGNORECASE,
    )
    title = re.sub(r"^(?:an?\s+)", "", title, flags=re.IGNORECASE)
    title = re.sub(
        r"^(?:personal|work|birthdays|holidays|general)\s+",
        "",
        title,
        flags=re.IGNORECASE,
    )
    # Keep meeting/appointment as title candidate; strip only when more follows.
    title = re.sub(
        r"^(?:event|meeting|appointment|call|reminder)\s+",
        "",
        title,
        flags=re.IGNORECASE,
    )
    title = re.sub(
        r"\b(?:every|from|at|on|mon|tue|wed|thu|fri|sat|sun|monday|tuesday|wednesday|thursday|friday|saturday|sunday|tomorrow|today|tonight|for\s+\d).*$",
        "",
        title,
        flags=re.IGNORECASE,
    ).strip(" .,:-")
    title = re.sub(r"\s+for$", "", title, flags=re.IGNORECASE).strip(" .,:-")
    title = re.sub(r"\s+block$", "", title, flags=re.IGNORECASE).strip()
    if re.fullmatch(
        r"(?:personal|work|birthdays|holidays|general|event|meeting|appointment|call|reminder|for|to|a|an|the|\s)+",
        title,
        flags=re.IGNORECASE,
    ):
        title = ""
    if not title and noun_m:
        title = noun_m.group(1).capitalize()
    result_title = title[:60] if title else "New event"
    return result_title, category


def _split_schedule_segments(message: str) -> list[str]:
    parts = re.split(
        r"\balong\s*side\b|\balongside\b|\bas well as\b|\bfollowed by\b|\band then\b|;|\n",
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
        # Lunch/break "around midday" with no day → today; other undated → tomorrow.
        day = now if _is_lunch_or_break(lower) else now + timedelta(days=1)

    if time_range:
        (sh, sm), (eh, em) = time_range
        start = day.replace(hour=sh, minute=sm, second=0, microsecond=0)
        end = day.replace(hour=eh, minute=em, second=0, microsecond=0)
        if end <= start:
            end = end + timedelta(days=1)
    elif _is_lunch_or_break(lower) and not _has_explicit_clock(lower):
        # Default lunch / midday break: 12:00–13:00 local.
        start = day.replace(hour=12, minute=0, second=0, microsecond=0)
        end = day.replace(hour=13, minute=0, second=0, microsecond=0)
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
    # Sequential phrases ("followed by") without their own clock start after the prior event.
    for i in range(1, len(events)):
        if not _has_explicit_clock(segments[i]):
            prev_end = events[i - 1]["end_time"]
            dur = max(events[i]["end_time"] - events[i]["start_time"], 15 * 60_000)
            events[i]["start_time"] = prev_end
            events[i]["end_time"] = prev_end + dur
    if len(events) == 1:
        out = json.dumps(events[0])
    else:
        out = json.dumps({"events": events})
    return out


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
    minutes: int | None = None
    if hours is not None:
        minutes = int(round(hours * 60))
    m = re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower)
    if m and hours is None:
        minutes = int(m.group(1))

    if "tomorrow" in lower:
        day = now + timedelta(days=1)
        start, end = _local_day_bounds_ms(day)
    elif "today" in lower:
        start, end = _local_day_bounds_ms(now)
        start = max(start, int(now.timestamp() * 1000))
    elif "next week" in lower:
        days_until_monday = (7 - now.weekday()) % 7
        if days_until_monday == 0:
            days_until_monday = 7
        week_start = now + timedelta(days=days_until_monday)
        week_end = week_start + timedelta(days=6)
        start = _local_day_bounds_ms(week_start)[0]
        end = _local_day_bounds_ms(week_end)[1] - 1
    else:
        start = int(now.timestamp() * 1000)
        end = _end_of_local_week_ms(now)

    payload: dict[str, Any] = {
        "start": start,
        "end": end,
        "limit": 5,
        # Default slot length for open availability questions.
        "duration_minutes": max(minutes, 15) if minutes is not None else 60,
    }
    return json.dumps(payload)


def _heuristic_block_time_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    hours = _parse_duration_hours(lower)
    minutes: int | None = None
    if hours is not None:
        minutes = int(round(hours * 60))
    m = re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower)
    if m and hours is None:
        minutes = int(m.group(1))
    title = "Focus"
    if "cod" in lower:
        title = "Coding"
    elif "writ" in lower:
        title = "Writing"
    elif "read" in lower:
        title = "Reading"
    m_title = re.search(
        r"\bblock\b(?:\s+\d+\s*(?:hours?|hrs?|minutes?|mins?))?\s+(?:for\s+)?(.+)$",
        message.strip(),
        flags=re.IGNORECASE,
    )
    if m_title:
        raw = re.sub(
            r"\b(?:this week|today|tomorrow|for)\b.*$",
            "",
            m_title.group(1),
            flags=re.IGNORECASE,
        ).strip(" .,:-")
        if raw and len(raw) < 60:
            title = raw[:1].upper() + raw[1:]
    payload: dict[str, Any] = {
        "title": title,
        "start": int(now.timestamp() * 1000),
        "end": _end_of_local_week_ms(now),
        "apply": False,
    }
    if minutes is not None:
        payload["duration_minutes"] = max(minutes, 30)
    return json.dumps(payload)


def _looks_like_week_plan(message: str) -> bool:
    """True for 'plan my week' / multi-session week packing requests."""
    lower = message.lower()
    if re.search(r"\bplan (?:out )?(?:my )?week\b", lower):
        return True
    if re.search(r"\bschedule (?:out )?(?:my )?week\b", lower):
        return True
    session_hits = len(re.findall(r"\bsessions?\b", lower))
    if session_hits >= 2 and re.search(
        r"\b(?:week|weekend|after work|include)\b", lower
    ):
        return True
    return False


def _title_case_activity(raw: str) -> str:
    title = re.sub(r"\s+", " ", (raw or "").strip(" .,:-")).strip()
    title = re.sub(
        r"^(?:go|do|book|for|the|my|a|some|include)\s+",
        "",
        title,
        flags=re.IGNORECASE,
    ).strip()
    lower = title.lower()
    if re.search(r"\bcyber", lower) and re.search(r"\bstudy", lower):
        return "Cybersecurity study"
    if re.search(r"\bcyber", lower):
        return "Cybersecurity study"
    for key in sorted(_ACTIVITY_ALIASES, key=len, reverse=True):
        if re.search(rf"\b{re.escape(key)}\b", lower):
            return _ACTIVITY_ALIASES[key]
    if not title or lower in _JUNK_TITLES:
        return "Task"
    return title[:1].upper() + title[1:80]


def _parse_count_token(raw: str) -> int | None:
    s = (raw or "").strip().lower()
    if not s:
        return None
    if s.isdigit():
        return min(max(int(s), 1), 14)
    return _COUNT_WORDS.get(s)


def _parse_session_minutes_near(text: str) -> int | None:
    """Parse duration from a session clause fragment."""
    hours = _parse_duration_hours(text)
    if hours is not None:
        return max(int(round(hours * 60)), 15)
    # "1:30hr" / "1:30" without requiring the word hours (climbing style).
    m = re.search(r"\b(\d+)\s*:\s*(\d+)\s*(?:hours?|hrs?|hr)?\b", text, flags=re.IGNORECASE)
    if m:
        return max(int(m.group(1)) * 60 + int(m.group(2)), 15)
    m = re.search(r"\b(\d+)\s*(?:minutes?|mins?|min)\b", text, flags=re.IGNORECASE)
    if m:
        return max(int(m.group(1)), 15)
    return None


def _parse_multi_schedule_tasks(message: str) -> list[dict[str, Any]] | None:
    """2+ distinct schedule items, or None (used as a routing signal)."""
    items = _extract_schedule_items(message)
    return items if items and len(items) >= 2 else None


def _extract_schedule_items(message: str) -> list[dict[str, Any]] | None:
    """NL slot fill: N×activity clauses, with or without duration.

    Glue like 'to include' / 'lets plan my week' is ignored. Duration is
    optional — Core defaults session length.
    """
    lower = message.lower()
    tasks: list[dict[str, Any]] = []
    seen_spans: list[tuple[int, int]] = []

    def _overlaps(start: int, end: int) -> bool:
        return any(not (end <= a or start >= b) for a, b in seen_spans)

    def _weekend_near(start: int) -> bool:
        window = lower[max(0, start - 40) : start + 12]
        return bool(re.search(r"\bweekend\b", window))

    def _push(title: str, count: int, minutes: int | None, start: int, end: int) -> None:
        if _overlaps(start, end):
            return
        cleaned = _title_case_activity(title)
        if cleaned.lower() in _JUNK_TITLES or cleaned == "Task":
            return
        item: dict[str, Any] = {
            "title": cleaned,
            "count": min(max(count, 1), 14),
            "prefer_spread": True,
        }
        if minutes is not None:
            item["duration_minutes"] = max(minutes, 15)
        if _weekend_near(start):
            item["prefer_after_work"] = False
            item["description"] = "prefer weekend"
        elif "after work" in lower:
            item["prefer_after_work"] = True
        tasks.append(item)
        seen_spans.append((start, end))

    count_word = "|".join(sorted(_COUNT_WORDS, key=len, reverse=True))

    patterns: list[tuple[re.Pattern[str], str]] = [
        (
            re.compile(
                r"\b(\d+)\s+(\d+)\s*(?:min(?:ute)?s?|mins?)\s+"
                r"([\w][\w\s]{0,40}?)\s+sessions?"
                r"(?:\s+for\s+([\w][\w\s]{0,60}))?",
                flags=re.IGNORECASE,
            ),
            "count_min_title",
        ),
        (
            re.compile(
                r"\b(\d+)\s+(\d+(?:\.\d+)?)\s*(?:hours?|hrs?|hr)\s+"
                r"([\w][\w\s]{0,40}?)\s+sessions?"
                r"(?:\s+for\s+([\w][\w\s]{0,60}))?",
                flags=re.IGNORECASE,
            ),
            "count_hour_title",
        ),
        (
            re.compile(
                rf"\b(\d+|{count_word})\s+([\w][\w\s]{{0,40}}?)\s+sessions?\s+for\s+"
                r"(\d+\s*:\s*\d+|\d+(?:\.\d+)?)\s*(?:hours?|hrs?|hr|minutes?|mins?|min)?",
                flags=re.IGNORECASE,
            ),
            "count_title_for_dur",
        ),
        # "2 climbing sessions" / "two gym sessions" — duration optional.
        (
            re.compile(
                rf"\b(\d+|{count_word})\s+([\w][\w-]{{0,40}}?)\s+sessions?\b",
                flags=re.IGNORECASE,
            ),
            "count_title_sessions",
        ),
        (
            re.compile(
                rf"\b(\d+)\s+times?\b[\w\s]{{0,48}}?\b({_ACTIVITY_ALT})\b",
                flags=re.IGNORECASE,
            ),
            "n_times_then_activity",
        ),
        (
            re.compile(
                rf"\b({_ACTIVITY_ALT})\b[\w\s]{{0,40}}?\b(\d+)\s+times?\b",
                flags=re.IGNORECASE,
            ),
            "activity_then_n_times",
        ),
        (
            re.compile(
                rf"\b(twice|thrice|once)\b[\w\s]{{0,40}}?\b({_ACTIVITY_ALT})\b",
                flags=re.IGNORECASE,
            ),
            "word_then_activity",
        ),
        (
            re.compile(
                rf"\b({_ACTIVITY_ALT})\b[\w\s]{{0,24}}?\b(twice|thrice|once)\b",
                flags=re.IGNORECASE,
            ),
            "activity_then_word",
        ),
    ]

    for cre, kind in patterns:
        for m in cre.finditer(message):
            minutes: int | None = None
            if kind == "count_min_title":
                count = _parse_count_token(m.group(1)) or 1
                minutes = max(int(m.group(2)), 15)
                title_raw = m.group(4) or m.group(3)
            elif kind == "count_hour_title":
                count = _parse_count_token(m.group(1)) or 1
                minutes = max(int(round(float(m.group(2)) * 60)), 15)
                title_raw = m.group(4) or m.group(3)
            elif kind == "count_title_for_dur":
                count = _parse_count_token(m.group(1)) or 1
                minutes = _parse_session_minutes_near(m.group(3)) or _parse_session_minutes_near(
                    m.group(0)
                )
                if minutes is None:
                    continue
                title_raw = m.group(2)
            elif kind == "count_title_sessions":
                count = _parse_count_token(m.group(1)) or 1
                title_raw = m.group(2)
            elif kind == "n_times_then_activity":
                count = _parse_count_token(m.group(1)) or 1
                title_raw = m.group(2)
            elif kind == "activity_then_n_times":
                title_raw = m.group(1)
                count = _parse_count_token(m.group(2)) or 1
            elif kind == "word_then_activity":
                count = _parse_count_token(m.group(1)) or 1
                title_raw = m.group(2)
            else:
                title_raw = m.group(1)
                count = _parse_count_token(m.group(2)) or 1
            _push(title_raw, count, minutes, m.start(), m.end())

    if _looks_like_week_plan(message) or re.search(
        r"\b(?:with|include|including)\b", lower
    ):
        mentioned = {t["title"].lower() for t in tasks}
        for m in re.finditer(rf"\b({_ACTIVITY_ALT})\b", message, flags=re.IGNORECASE):
            # "after work" is a constraint, not an activity named Work.
            prev = lower[max(0, m.start() - 8) : m.start()]
            if m.group(1).lower() == "work" and prev.rstrip().endswith("after"):
                continue
            if _overlaps(m.start(), m.end()):
                continue
            title = _title_case_activity(m.group(1))
            if title.lower() in mentioned or title.lower() in _JUNK_TITLES:
                continue
            _push(m.group(1), 1, None, m.start(), m.end())
            mentioned.add(title.lower())

    return tasks or None


def _heuristic_schedule_task_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()

    if "tomorrow" in lower:
        deadline_day = now + timedelta(days=1)
        deadline = _local_day_bounds_ms(deadline_day)[1] - 1
        start = _local_day_bounds_ms(deadline_day)[0]
    elif "today" in lower:
        start = max(int(now.timestamp() * 1000), _local_day_bounds_ms(now)[0])
        deadline = _local_day_bounds_ms(now)[1] - 1
    elif "next week" in lower:
        days_until_monday = (7 - now.weekday()) % 7
        if days_until_monday == 0:
            days_until_monday = 7
        week_start = now + timedelta(days=days_until_monday)
        start = _local_day_bounds_ms(week_start)[0]
        next_week_end = week_start + timedelta(days=6)
        deadline = _local_day_bounds_ms(next_week_end)[1] - 1
    else:
        deadline = _end_of_local_week_ms(now)
        start = int(now.timestamp() * 1000)

    multi = _extract_schedule_items(message)
    if multi:
        payload: dict[str, Any] = {
            "tasks": multi,
            "deadline": deadline,
            "priority": "high" if "urgent" in lower or "asap" in lower else "normal",
            "flexibility": "flexible",
            "start": start,
            "end": deadline,
            "apply": False,
        }
        if "after work" in lower:
            payload["prefer_after_work"] = True
        return json.dumps(payload)

    hours = _parse_duration_hours(lower)
    minutes: int | None = None
    if hours is not None:
        minutes = int(round(hours * 60))
    m = re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower)
    if m and hours is None:
        minutes = int(m.group(1))

    count = _parse_occurrence_count(lower)
    # Repeated activities without an explicit duration still need Clarification —
    # do not invent a session length.

    title_guess, _cat = _infer_title_category(message)
    if title_guess and title_guess.lower() not in (
        "book",
        "new event",
        "event",
        "meeting",
        "work",
        "include",
        "task",
    ):
        title = title_guess
    else:
        title = re.sub(
            r"^(?:please\s+)?(?:finish|schedule|complete|do|book|i want to(?:\s+go|\s+do|\s+get to|\s+book)?|make time for|fit in|plan (?:out )?(?:my )?week|book (?:some |a )?time (?:this week |today |tomorrow )?for)\s+(?:the\s+|my\s+|a\s+|some\s+)?",
            "",
            message.strip(),
            flags=re.IGNORECASE,
        )
        title = re.sub(
            r"\b(?:this week|next week|today|tomorrow|,?\s*\d+\s*(?:hours?|hrs?|minutes?|mins?)|\d+\s+times?|once|twice|thrice|prefer_spread|after work).*$",
            "",
            title,
            flags=re.IGNORECASE,
        ).strip(" .,:-")
        if not title:
            title = "Task"
        title = _title_case_activity(title)

    payload = {
        "title": title[:80],
        "deadline": deadline,
        "priority": "high" if "urgent" in lower or "asap" in lower else "normal",
        "flexibility": "flexible",
        "start": start,
        "end": deadline,
        "apply": False,
    }
    if minutes is not None:
        payload["duration_minutes"] = max(minutes, 15)
    if count > 1:
        payload["count"] = count
        payload["prefer_spread"] = True
    if "after work" in lower:
        payload["prefer_after_work"] = True
    return json.dumps(payload)


def _schedule_payload_to_organize(st: dict[str, Any], message: str) -> dict[str, Any]:
    items: list[dict[str, Any]] = []
    if isinstance(st.get("tasks"), list) and st["tasks"]:
        for t in st["tasks"]:
            if not isinstance(t, dict):
                continue
            item: dict[str, Any] = {"title": t.get("title") or "Task"}
            if t.get("duration_minutes"):
                item["duration_minutes"] = t["duration_minutes"]
            if t.get("count"):
                item["count"] = t["count"]
            if t.get("prefer_after_work"):
                item["when"] = "after_work"
            if "weekend" in str(t.get("description") or "").lower():
                item["when"] = "weekend"
            items.append(item)
    elif st.get("title"):
        item = {"title": st["title"]}
        if st.get("duration_minutes"):
            item["duration_minutes"] = st["duration_minutes"]
        if st.get("count"):
            item["count"] = st["count"]
        if st.get("prefer_after_work"):
            item["when"] = "after_work"
        items.append(item)
    constraints: list[str] = []
    if st.get("prefer_after_work") or re.search(r"after work", message, flags=re.I):
        constraints.append("after_work")
    lower = message.lower()
    window = "this_week"
    parsed_when = _parse_when_label(message)
    if parsed_when and not re.search(r"\bthis week\b", lower):
        window = parsed_when
    elif re.search(r"\btomorrow\b", lower) and not re.search(r"\bthis week\b", lower):
        window = "tomorrow"
    elif re.search(r"\btoday\b", lower) and not re.search(r"\bthis week\b", lower):
        window = "today"
    elif re.search(r"\bon sunday\b", lower) and not re.search(r"\bthis week\b", lower):
        window = "sunday"
    elif re.search(r"\bweekend\b", lower) and not re.search(
        r"\bthis week\b", lower
    ):
        window = "weekend"
    return {
        "window": window,
        "mode": "propose",
        "constraints": constraints,
        "items": items,
    }


def _heuristic_organize_input(message: str) -> str:
    st = json.loads(_heuristic_schedule_task_input(message))
    # Block-time style: title from block heuristic when schedule title is generic.
    if _matches_any(message, CALENDAR_BLOCK_TIME) and not st.get("tasks"):
        try:
            block = json.loads(_heuristic_block_time_input(message))
            if block.get("title"):
                st["title"] = block["title"]
            if block.get("duration_minutes") and "duration_minutes" not in st:
                st["duration_minutes"] = block["duration_minutes"]
        except (json.JSONDecodeError, TypeError):
            pass
    if _matches_any(message, CALENDAR_PLAN_DAY) and not st.get("tasks"):
        try:
            day = json.loads(_heuristic_plan_day_input(message))
            if day.get("tasks"):
                st["tasks"] = day["tasks"]
        except (json.JSONDecodeError, TypeError):
            pass
    return json.dumps(_schedule_payload_to_organize(st, message))


def _parse_when_label(message: str) -> str | None:
    lower = message.lower()
    if "next week" in lower or "next_week" in lower:
        return "next_week"
    if "weekend" in lower:
        return "weekend"
    if (
        "this week" in lower
        or "this_week" in lower
        or ("the week" in lower and "weekend" not in lower)
        or "all week" in lower
        or "whole week" in lower
        or "entire week" in lower
    ):
        return "this_week"
    if "tomorrow" in lower:
        return "tomorrow"
    if (
        "not today" in lower
        or "not tonight" in lower
        or "n't today" in lower
        or "except today" in lower
    ):
        return "not_today"
    if "today" in lower or "tonight" in lower:
        return "today"
    for day in (
        "monday",
        "tuesday",
        "wednesday",
        "thursday",
        "friday",
        "saturday",
        "sunday",
    ):
        if re.search(rf"\b{day}\b", lower):
            return day
    return None


def _looks_like_docs_format(message: str) -> bool:
    lower = message.lower()
    if re.search(r"~/|(?:/users/|/home/)", lower):
        return False
    if re.search(
        r"\b(?:reformat|better format|format better|improve (?:the )?format|cleaner format|tidier format)\b",
        lower,
    ):
        if re.search(r"\b(?:make|create|new)\s+(?:a\s+)?(?:document|doc)\b", lower) and ":" in message:
            return False
        return True
    return False


def _heuristic_docs_format_input(message: str) -> str:
    named = re.search(
        r"(?i)(?:edit|format|reformat)\s+[\"']?([^\s\"'`,:!]+)",
        message,
    )
    if named:
        token = named.group(1).strip()
        if token.lower() not in {"the", "this", "my", "a", "an"}:
            return json.dumps({"id": token})
    titled = re.search(
        r"(?i)(?:document|doc|note)\s+(?:called|named|titled)\s+[\"']?([^\s\"'`,:!]+)",
        message,
    )
    if titled:
        return json.dumps({"id": titled.group(1).strip()})
    return "{}"


def _looks_like_docs_upsert(message: str) -> bool:
    lower = message.lower()
    if re.search(r"~/|(?:/users/|/home/)", lower):
        return False
    if re.search(r"\b(?:document|doc|note)\s+(?:called|named|titled)\b", lower):
        return True
    if re.search(
        r"\b(?:make|create|add|save|new)\s+(?:a\s+|an\s+)?(?:document|doc)\b",
        lower,
    ):
        return True
    return False


def _heuristic_docs_upsert_input(message: str) -> str:
    title = None
    named = re.search(
        r"(?i)(?:document|doc|note)\s+(?:called|named|titled)\s+[\"']?([^\s\"'`,:!]+)",
        message,
    )
    if named:
        title = named.group(1).strip()
    content = ""
    after = message
    if title:
        idx = message.lower().find(title.lower())
        if idx >= 0:
            after = message[idx + len(title) :]
    for match in re.finditer(r":", after):
        rest = after[match.end() :].lstrip()
        if rest.startswith("//"):
            continue
        if len(rest) >= 12:
            content = rest
            break
    payload: dict[str, str] = {}
    if title:
        payload["title"] = title
    if content:
        payload["content"] = content
    return json.dumps(payload) if payload else "{}"


def _looks_like_list_dir(message: str) -> bool:
    lower = message.lower()
    if re.search(r"\bcalend", lower):
        return False
    if re.search(r"~/|(?:\b(?:downloads|desktop|documents|folder|directory)\b)", lower) and re.search(
        r"\b(?:show|list|what(?:'s|s| is) in)\b",
        lower,
    ):
        return True
    return bool(
        re.search(
            r"\b(?:list|show)\b.+\b(?:files?|folder|directory)\b",
            lower,
        )
    )


def _heuristic_list_dir_input(message: str) -> str:
    m = re.search(r"(~/[^\s]+|/(?:Users|home)/[^\s]+)", message)
    if m:
        return json.dumps({"path": m.group(1)})
    lower = message.lower()
    for name, folder in (
        ("downloads", "Downloads"),
        ("desktop", "Desktop"),
        ("documents", "Documents"),
    ):
        if name in lower:
            return json.dumps({"path": folder})
    return json.dumps({"path": "."})


def _looks_like_calendar_look(message: str) -> bool:
    if _looks_like_week_plan(message):
        return False
    if _looks_like_list_dir(message):
        return False
    lower = message.lower()
    if re.search(r"\bplan (?:out )?(?:my |the )?(?:week|day)\b", lower):
        return False
    if re.search(r"\bplanning\b", lower):
        return False
    if _matches_any(message, CALENDAR_QUERY_WORK):
        return True
    if _matches_any(message, CALENDAR_FREE_TIME):
        return True
    if _matches_any(message, CALENDAR_QUERY_TODAY + CALENDAR_QUERY_TOMORROW + CALENDAR_QUERY_WEEK):
        return True
    return _matches_any(message, CALENDAR_LOOK_CUES)


def _heuristic_look_input(
    message: str, focus: str | None = None, when: str | None = None
) -> str:
    lower = message.lower()
    work_q = _matches_any(message, CALENDAR_QUERY_WORK)
    if when is None:
        when = _parse_when_label(message)
        if when is None:
            if work_q or re.search(r"\bplans\b", lower):
                when = "this_week"
            else:
                when = "today"
    if focus is None:
        if work_q:
            focus = "work"
        else:
            focus = "free" if _matches_any(message, CALENDAR_FREE_TIME) else "events"
    payload: dict[str, Any] = {"when": when, "focus": focus}
    hours = _parse_duration_hours(lower)
    if hours is not None:
        payload["duration_minutes"] = max(int(round(hours * 60)), 15)
    elif focus == "free" and re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower):
        payload["duration_minutes"] = int(
            re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", lower).group(1)
        )
    elif focus == "free" and not re.search(
        r"\b(\d+|hour|hr|minute|min)\b", lower
    ):
        payload["duration_minutes"] = 60
    return json.dumps(payload)


def _heuristic_pin_input(message: str) -> str:
    raw = json.loads(_heuristic_create_event_input(message))
    if isinstance(raw.get("events"), list) and raw["events"]:
        first = raw["events"][0]
        payload = {
            "action": "create",
            "title": first.get("title") or raw.get("title") or "Event",
            "start": str(first.get("start_time") or ""),
            "end": str(first.get("end_time") or ""),
        }
        if first.get("category"):
            payload["category"] = first["category"]
        return json.dumps(payload)
    payload = {
        "action": "create",
        "title": raw.get("title") or "Event",
    }
    if raw.get("start_time") is not None:
        payload["start"] = str(raw["start_time"])
    if raw.get("end_time") is not None:
        payload["end"] = str(raw["end_time"])
    if raw.get("category"):
        payload["category"] = raw["category"]
    return json.dumps(payload)


def _parse_occurrence_count(message: str) -> int:
    lower = message.lower()
    m = re.search(r"\b(\d+)\s+times?\b", lower)
    if m:
        return min(max(int(m.group(1)), 1), 14)
    if re.search(r"\btwice\b", lower):
        return 2
    if re.search(r"\bthrice\b", lower):
        return 3
    if re.search(r"\bonce\b", lower):
        return 1
    return 1


def _resolve_plan_day_ms(message: str, now: datetime) -> int:
    lower = message.lower()
    if "tomorrow" in lower:
        day = now + timedelta(days=1)
    elif "today" in lower:
        day = now
    else:
        weekday_names = [
            ("monday", "MO"),
            ("tuesday", "TU"),
            ("wednesday", "WE"),
            ("thursday", "TH"),
            ("friday", "FR"),
            ("saturday", "SA"),
            ("sunday", "SU"),
        ]
        day = now
        for name, code in weekday_names:
            if re.search(rf"\b{name}\b", lower):
                day = _next_weekday(now, [code])
                break
    return int(day.replace(hour=12, minute=0, second=0, microsecond=0).timestamp() * 1000)


def _heuristic_plan_day_input(message: str) -> str:
    lower = message.lower()
    now = datetime.now().astimezone()
    day_ms = _resolve_plan_day_ms(message, now)

    rest = re.sub(
        r"^(?:please\s+)?(?:plan|schedule|organi[sz]e|map out|fill)\s+(?:out\s+)?(?:my\s+|the\s+)?day\s*(?:for\s+)?(?:tomorrow|today)?\s*[,:]?\s*",
        "",
        message.strip(),
        flags=re.IGNORECASE,
    )
    rest = re.sub(
        r"^(?:please\s+)?i want to\s+",
        "",
        rest,
        flags=re.IGNORECASE,
    )
    rest = re.sub(
        r"\bon\s+(?:sunday|monday|tuesday|wednesday|thursday|friday|saturday)\b",
        "",
        rest,
        flags=re.IGNORECASE,
    )
    rest = re.sub(
        r"\b(?:this|next)\s+(?:sunday|saturday|monday|tuesday|wednesday|thursday|friday)\b",
        "",
        rest,
        flags=re.IGNORECASE,
    )

    tasks: list[dict[str, Any]] = []

    spark_m = re.search(
        r"\b(\d+|two|three|a couple of|a few)\s+(?:of\s+)?(?:my\s+)?sparks?\b",
        rest,
        flags=re.IGNORECASE,
    )
    if spark_m:
        raw = spark_m.group(1).lower()
        word_counts = {"two": 2, "three": 3, "a couple of": 2, "a few": 3}
        n = word_counts.get(raw, None)
        if n is None:
            n = min(max(int(raw), 1), 5) if raw.isdigit() else 2
        for i in range(n):
            tasks.append(
                {
                    "title": f"Spark work {i + 1}" if n > 1 else "Spark work",
                    "flexibility": "flexible",
                    "priority": "normal",
                }
            )
        rest = rest[: spark_m.start()] + rest[spark_m.end() :]
    elif re.search(r"\bsparks?\b", rest, flags=re.IGNORECASE):
        tasks.append(
            {
                "title": "Spark work",
                "flexibility": "flexible",
                "priority": "normal",
            }
        )
        rest = re.sub(r"\b(?:my\s+)?sparks?\b", "", rest, flags=re.IGNORECASE)

    parts = re.split(r"\s*(?:,| and | & |\+|plus)\s*", rest, flags=re.IGNORECASE)
    for part in parts:
        name = part.strip(" .,:-")
        if not name or len(name) < 2:
            continue
        if re.match(
            r"^(?:tomorrow|today|please|do|go|etc|and)$",
            name,
            flags=re.IGNORECASE,
        ):
            continue
        hours = _parse_duration_hours(name)
        clean = re.sub(
            r"\b\d+\s*(?:hours?|hrs?|minutes?|mins?)\b",
            "",
            name,
            flags=re.IGNORECASE,
        ).strip(" .,:-")
        clean = re.sub(r"^(?:go|do)\s+", "", clean, flags=re.IGNORECASE).strip()
        if not clean or len(clean) < 2:
            continue
        if re.match(r"^spark\b", clean, flags=re.IGNORECASE):
            continue
        task: dict[str, Any] = {
            "title": clean[:60],
            "flexibility": "flexible",
            "priority": "normal",
        }
        # Only set duration when the user stated a length — Clarification asks otherwise.
        if hours is not None:
            task["duration_minutes"] = max(int(round(hours * 60)), 30)
        else:
            m_mins = re.search(r"\b(\d+)\s*(?:minutes?|mins?)\b", name, flags=re.IGNORECASE)
            if m_mins:
                task["duration_minutes"] = max(int(m_mins.group(1)), 15)
        tasks.append(task)

    return json.dumps(
        {
            "day": day_ms,
            "tasks": tasks,
            "include_breaks": True,
            # Imperative "plan my day" applies; soft desire language proposes first.
            "apply": bool(
                re.search(
                    r"\b(?:plan|schedule|organi[sz]e|map out|fill)\b.+\b(?:day|tomorrow)\b",
                    message,
                    flags=re.IGNORECASE,
                )
            ),
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
    if _looks_like_docs_format(message):
        return PlanResponse(
            intent="tool_use",
            tool="docs.format",
            tool_input=_heuristic_docs_format_input(message),
            reasoning="User asked to improve an in-app Document's format.",
            response=None,
        )
    if _looks_like_docs_upsert(message):
        return PlanResponse(
            intent="tool_use",
            tool="docs.upsert",
            tool_input=_heuristic_docs_upsert_input(message),
            reasoning="User asked to create or update an in-app Document.",
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
    if _looks_like_list_dir(message):
        return PlanResponse(
            intent="tool_use",
            tool="list_dir",
            tool_input=_heuristic_list_dir_input(message),
            reasoning="User asked to list a folder.",
            response=None,
        )
    # Scheduling intelligence — before agenda queries so "free tomorrow" ≠ look today.
    if _matches_any(message, CALENDAR_FREE_TIME):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.look",
            tool_input=_heuristic_look_input(message, focus="free"),
            reasoning="User asked for free/available time.",
            response=None,
        )
    # Week packing / multi-session plans before single-day organize.
    if _looks_like_week_plan(message) or _extract_schedule_items(message):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.organize",
            tool_input=_heuristic_organize_input(message),
            reasoning="User asked to plan/schedule multiple sessions across the week.",
            response=None,
        )
    if _matches_any(message, CALENDAR_PLAN_DAY):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.organize",
            tool_input=_heuristic_organize_input(message),
            reasoning="User asked to plan their day.",
            response=None,
        )
    # Multi-occurrence ("3 times this week") before pin.
    if re.search(r"\b(?:\d+\s+times?|twice|thrice)\b", message, flags=re.IGNORECASE) and (
        _matches_any(message, CALENDAR_SCHEDULE_TASK)
        or re.search(r"\b(?:this|next)\s+(?:week|month)\b", message, flags=re.IGNORECASE)
        or re.search(r"\bi want to\b", message, flags=re.IGNORECASE)
    ):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.organize",
            tool_input=_heuristic_organize_input(message),
            reasoning="User asked to schedule an activity multiple times.",
            response=None,
        )
    # Clock pin before focus-block organize so "research block at 11" stays a pin.
    if _matches_any(message, CALENDAR_CREATE) and not _is_soft_slot_booking(message):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.pin",
            tool_input=_heuristic_pin_input(message),
            reasoning="User asked to pin a calendar event at a clock time.",
            response=None,
        )
    if _matches_any(message, CALENDAR_BLOCK_TIME):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.organize",
            tool_input=_heuristic_organize_input(message),
            reasoning="User asked to block focus time.",
            response=None,
        )
    if _matches_any(message, CALENDAR_SCHEDULE_TASK):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.organize",
            tool_input=_heuristic_organize_input(message),
            reasoning="User asked to schedule a task into free time.",
            response=None,
        )
    if _matches_any(message, CALENDAR_CAPACITY):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.look",
            tool_input=_heuristic_look_input(message),
            reasoning="User asked about daily capacity or summary.",
            response=None,
        )
    if _matches_any(message, CALENDAR_QUERY_WORK):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.look",
            tool_input=_heuristic_look_input(message, focus="work"),
            reasoning="User asked about work hours.",
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
            tool="calendar.look",
            tool_input=json.dumps(
                {
                    "when": "this_week",
                    "focus": "events",
                    "query": q.strip() or message.strip(),
                }
            ),
            reasoning="User asked to search the calendar.",
            response=None,
        )
    if _matches_any(message, CALENDAR_DELETE):
        delete_payload = json.loads(_heuristic_delete_event_input(message))
        if isinstance(delete_payload, dict):
            delete_payload["action"] = "delete"
            if "query" in delete_payload and "title" not in delete_payload:
                delete_payload["title"] = delete_payload.get("query")
        return PlanResponse(
            intent="tool_use",
            tool="calendar.pin",
            tool_input=json.dumps(delete_payload),
            reasoning="User asked to remove calendar event(s).",
            response=None,
        )
    if _looks_like_calendar_look(message):
        return PlanResponse(
            intent="tool_use",
            tool="calendar.look",
            tool_input=_heuristic_look_input(message),
            reasoning="User asked what's on / planned.",
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
        return f"I got your question: \"{text}\". Want to rephrase or add a bit more detail?"
    return f"Noted: \"{text}\". Want to expand on that, or should I take a next step?"


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
    next_action = (plan.next or "").strip().lower() or None

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

    if next_action not in ("ask", "tool", "finish"):
        if intent == "tool_use" and tool:
            next_action = "tool"
        elif intent == "chat":
            next_action = "finish"
        else:
            next_action = "finish"

    plan.intent = intent
    plan.tool = tool
    plan.tool_input = tool_input
    plan.next = next_action
    return apply_respond_mode(plan)


def heuristic_classify_tool(message: str) -> Optional[str]:
    """Repair helper: best-effort tool name from heuristics (no arg fill)."""
    plan = normalize_plan(_heuristic_plan(message))
    if plan.intent != "tool_use" or not plan.tool:
        return None
    if plan.tool in _HEURISTIC_SLOW_TOOLS:
        return None
    if plan.tool in ("calendar.create_event", "calendar.pin") and _is_soft_slot_booking(
        message
    ):
        return "calendar.organize"
    return plan.tool


def repair_classified_tool(
    intent: str,
    tool: Optional[str],
    message: str,
    history: Optional[list] = None,
) -> tuple[str, Optional[str]]:
    """Override tool *name* only when classify clearly missed. Never builds args."""
    tool = TOOL_ALIASES.get(tool, tool) if isinstance(tool, str) and tool else tool

    if intent == "tool_use" and tool in (
        "calendar.create_event",
        "calendar.pin",
    ) and _is_soft_slot_booking(message):
        return "tool_use", "calendar.organize"

    calendar_family = bool(
        tool
        and (
            tool.startswith("calendar.")
            or tool.startswith("dream.")
            or tool.startswith("work.")
            or tool.startswith("lifestyle.")
        )
    )
    if intent == "tool_use" and calendar_family:
        return intent, tool

    if _looks_like_docs_format(message) and tool in (
        None,
        "docs.upsert",
        "write_file",
        "edit_file",
        "save_spark",
        "coder.run",
    ):
        return "tool_use", "docs.format"

    if _looks_like_docs_upsert(message) and tool in (
        None,
        "write_file",
        "edit_file",
        "save_spark",
        "coder.run",
    ):
        return "tool_use", "docs.upsert"

    scheduling_patterns = (
        CALENDAR_FREE_TIME
        + CALENDAR_PLAN_DAY
        + CALENDAR_BLOCK_TIME
        + CALENDAR_SCHEDULE_TASK
        + CALENDAR_CAPACITY
    )
    if _matches_any(message, scheduling_patterns):
        repaired = heuristic_classify_tool(message)
        if repaired:
            return "tool_use", repaired

    if intent == "chat":
        ask_when = re.search(
            r"\b(?:ask me|when|what time|how long|which day)\b",
            message,
            flags=re.IGNORECASE,
        )
        if ask_when and history:
            for item in reversed(history):
                role = item.get("role") if isinstance(item, dict) else getattr(item, "role", "")
                content = (
                    item.get("content", "")
                    if isinstance(item, dict)
                    else getattr(item, "content", "")
                )
                if role != "user":
                    continue
                prior = (content or "").strip()
                if _matches_any(prior, CALENDAR_SCHEDULE_TASK) or re.search(
                    rf"\b(?:{_CAL_ACTIVITY})\b", prior, flags=re.IGNORECASE
                ):
                    return "tool_use", "calendar.organize"
                break

    if _matches_any(message, CALENDAR_CREATE) or _matches_any(message, CALENDAR_DELETE):
        repaired = heuristic_classify_tool(message)
        if repaired:
            return "tool_use", repaired

    lifestyle_patterns = DREAM_LOG + DREAM_SEARCH + WORK_SALES + WORK_SET_HOURS + WORK_STATS
    if _matches_any(message, lifestyle_patterns):
        repaired = heuristic_classify_tool(message)
        if repaired:
            return "tool_use", repaired

    # Extra repair-only paraphrases (not the primary catalog / product router).
    if intent != "tool_use" or not tool:
        lower = message.lower()
        if _looks_like_week_plan(message) or _parse_multi_schedule_tasks(message):
            return "tool_use", "calendar.organize"
        if re.search(
            rf"\b(?:fit|schedule|book)\b.+\b(?:{_CAL_ACTIVITY}|session)\b",
            message,
            flags=re.IGNORECASE,
        ) or re.search(
            r"\bneed to schedule\b",
            message,
            flags=re.IGNORECASE,
        ):
            return "tool_use", "calendar.organize"
        if re.search(
            r"\b(?:meeting|appointment|call)\b.+\b(?:\d{1,2}\s*(?:am|pm)|friday|monday|tuesday|wednesday|thursday|saturday|sunday)\b",
            message,
            flags=re.IGNORECASE,
        ) or re.search(
            r"\b(?:friday|monday|tuesday|wednesday|thursday)\b.+\b\d{1,2}\s*(?:am|pm)\b",
            message,
            flags=re.IGNORECASE,
        ):
            return "tool_use", "calendar.pin"
        if _looks_like_docs_upsert(message):
            return "tool_use", "docs.upsert"
        if re.search(r"\bread (?:the )?file\b", lower) or re.search(
            r"\bopen (?:the )?file\b", lower
        ):
            return "tool_use", "read_file"
        if re.search(
            r"\b(?:list|show)\b.+\b(?:files?|folder|directory|downloads|desktop)\b",
            lower,
        ) or re.search(r"\bwhat'?s in\b.+/", lower):
            return "tool_use", "list_dir"
        if re.search(
            r"\b(?:implement|debug|refactor|fix (?:the |a |this )?bug)\b",
            lower,
        ) or re.search(r"\bhelp me (?:code|refactor|debug|implement)\b", lower):
            return "tool_use", "coder.run"

    return intent, tool


def heuristic_fill_for_tool(tool: str, message: str) -> str:
    """Repair fill when MLX fill is unavailable — prefer omit-unknowns shapes."""
    if tool in ("calendar.create_event", "calendar.pin") and _is_soft_slot_booking(
        message
    ):
        tool = "calendar.organize"
    plan = normalize_plan(_heuristic_plan(message))
    if plan.tool == tool and plan.tool_input:
        return plan.tool_input
    if tool == "calendar.organize":
        return _heuristic_organize_input(message)
    if tool == "calendar.look":
        return _heuristic_look_input(message)
    if tool == "calendar.pin":
        return _heuristic_pin_input(message)
    if tool == "echo":
        return message
    if tool == "docs.format":
        return _heuristic_docs_format_input(message)
    if tool == "docs.upsert":
        return _heuristic_docs_upsert_input(message)
    if tool == "coder.run":
        return json.dumps({"prompt": message, "focus": "focused"})
    return "{}"


def repair_schedule_task_title(plan: PlanResponse, message: str) -> PlanResponse:
    """Ensure schedule_task / organize keeps an activity title / multi-task list when MLX omitted it."""
    if plan.tool == "calendar.organize":
        try:
            data = json.loads(plan.tool_input or "{}")
        except (json.JSONDecodeError, TypeError):
            data = {}
        items = data.get("items") if isinstance(data, dict) else None
        thin = not items or (
            isinstance(items, list)
            and (
                len(items) < 2
                or any(not (i.get("title") if isinstance(i, dict) else None) for i in items)
            )
        )
        if thin and (
            _looks_like_week_plan(message) or _extract_schedule_items(message)
        ):
            plan.tool_input = _heuristic_organize_input(message)
            return plan
        if (not items) and re.search(rf"\b(?:{_CAL_ACTIVITY})\b", message, flags=re.I):
            plan.tool_input = _heuristic_organize_input(message)
        return plan
    if plan.tool != "calendar.schedule_task":
        return plan
    try:
        data = json.loads(plan.tool_input or "{}")
    except (json.JSONDecodeError, TypeError):
        data = {}
    if not isinstance(data, dict):
        data = {}
    title = data.get("title")
    title_ok = isinstance(title, str) and title.strip() and title.strip().lower() not in (
        "task",
        "book",
        "event",
        "meeting",
    )
    has_tasks = isinstance(data.get("tasks"), list) and len(data["tasks"]) > 0
    multi = _parse_multi_schedule_tasks(message)
    # Prefer multi-activity tasks[] when the user listed several sessions.
    if multi and (not has_tasks or len(data.get("tasks") or []) < 2):
        heuristic = json.loads(_heuristic_schedule_task_input(message))
        for key in ("tasks", "prefer_after_work", "start", "end", "deadline", "apply"):
            if key in heuristic:
                data[key] = heuristic[key]
        data.pop("title", None)
        data.pop("duration_minutes", None)
        data.pop("count", None)
        plan.tool_input = json.dumps(data)
        return plan
    if title_ok or has_tasks:
        return plan
    heuristic = json.loads(_heuristic_schedule_task_input(message))
    ht = heuristic.get("title")
    if isinstance(ht, str) and ht.strip():
        data["title"] = ht
        plan.tool_input = json.dumps(data)
    return plan


def repair_find_free_time_fill(plan: PlanResponse, message: str) -> PlanResponse:
    """Ensure look/free-time has a search window + default duration when MLX returns {}."""
    if plan.tool == "calendar.find_free_time":
        plan.tool = "calendar.look"
    if plan.tool != "calendar.look":
        return plan
    try:
        data = json.loads(plan.tool_input or "{}")
    except (json.JSONDecodeError, TypeError):
        data = {}
    if not isinstance(data, dict):
        data = {}
    heuristic = json.loads(_heuristic_free_time_input(message))
    changed = False

    def _as_int(v) -> int | None:
        if isinstance(v, bool):
            return None
        if isinstance(v, (int, float)):
            return int(v)
        if isinstance(v, str) and v.strip().isdigit():
            return int(v.strip())
        return None

    start = _as_int(data.get("start"))
    end = _as_int(data.get("end"))
    # MLX sometimes invents start==end (zero-width) — replace with heuristic window.
    if start is None or end is None or end <= start:
        data["start"] = heuristic.get("start")
        data["end"] = heuristic.get("end")
        changed = True
    if data.get("duration_minutes") is None and heuristic.get("duration_minutes") is not None:
        data["duration_minutes"] = heuristic["duration_minutes"]
        changed = True
    if data.get("limit") is None:
        data["limit"] = heuristic.get("limit", 5)
        changed = True
    if changed:
        plan.tool_input = json.dumps(data)
    return plan


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
            next=data.get("next"),
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
