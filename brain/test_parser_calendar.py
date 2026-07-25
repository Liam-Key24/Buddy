"""Heuristic calendar / lifestyle routing goldens.

Structure:
  - ROUTING: message → tool (intent coverage; add paraphrases as data)
  - EXTRACTION: payload shape for a few goldens per capability
  - NEGATIVE: chat / coder must not be stolen into calendar tools
  - CONTRACT: parse_plan / respond_mode plumbing
"""

from __future__ import annotations

import json
import time
from datetime import datetime
from typing import Any, Callable

from parser import (
    default_respond_mode,
    parse_plan,
    try_fast_heuristic_plan,
)

PayloadCheck = Callable[[dict[str, Any]], None]


# ---------------------------------------------------------------------------
# Routing goldens — one tool per intent family; paraphrases are free.
# ---------------------------------------------------------------------------

ROUTING_CASES: list[tuple[str, str]] = [
    # agenda
    ("What's on today?", "calendar.get_today"),
    ("What's happening today?", "calendar.get_today"),
    ("What's on my calendar today?", "calendar.get_today"),
    ("Show me tomorrow", "calendar.get_tomorrow"),
    ("Do I have anything this week?", "calendar.get_this_week"),
    # free time
    ("When am I free tomorrow for 2 hours?", "calendar.find_free_time"),
    ("Got any free time tomorrow?", "calendar.find_free_time"),
    ("Any open slots today?", "calendar.find_free_time"),
    # plan day
    ("Plan my day for tomorrow, tennis, bath and cooking dinner", "calendar.plan_day"),
    ("Organise my day for tomorrow, tennis and bath", "calendar.plan_day"),
    ("Map out my day: gym and grocery run", "calendar.plan_day"),
    ("I want to do two sparks and gym on Sunday", "calendar.plan_day"),
    (
        "I want to do two of my sparks, go gym and cook dinner on Sunday",
        "calendar.plan_day",
    ),
    # capacity
    ("What's my capacity today?", "calendar.get_capacity"),
    ("How busy is my day?", "calendar.get_capacity"),
    ("Am I overloaded today?", "calendar.get_capacity"),
    # create event
    ("Schedule a meeting tomorrow at 3pm", "calendar.create_event"),
    ("Pencil in a meeting tomorrow at 3pm", "calendar.create_event"),
    ("Set up an appointment for Friday at 10am", "calendar.create_event"),
    ("I have dentist tomorrow at 2pm", "calendar.create_event"),
    (
        "make a event called study for 3 hours at 11:00 tomorrow "
        "followed by a research block for 2 hours",
        "calendar.create_event",
    ),
    ("make a personal event tomorrow to code at 6pm-7pm", "calendar.create_event"),
    ("Add a lunch break around midday.", "calendar.create_event"),
    ("Add a lunch break tomorrow around midday.", "calendar.create_event"),
    # block time
    ("block 3 hours for coding", "calendar.block_time"),
    ("can you block 2 hours for coding", "calendar.block_time"),
    ("could you block off time for deep work", "calendar.block_time"),
    ("please hold 90 minutes for writing", "calendar.block_time"),
    ("reserve 1 hour for focus", "calendar.block_time"),
    ("set aside time for studying", "calendar.block_time"),
    # schedule task / multi-occurrence
    ("Finish the design report this week, 2 hours", "calendar.schedule_task"),
    ("Find time for the design report", "calendar.schedule_task"),
    ("Need to finish the report by Friday", "calendar.schedule_task"),
    ("I want to go climbing 3 times this week", "calendar.schedule_task"),
    ("Make time for yoga twice this week", "calendar.schedule_task"),
    # mutate / search
    ("Get rid of the meeting from my calendar", "calendar.delete_event"),
    ("Take it off my calendar", "calendar.delete_event"),
    ("Where is gym on my calendar?", "calendar.search_events"),
    ("Do I have a meeting on my calendar?", "calendar.search_events"),
    # sparks / lifestyle
    ("I got an idea for a camping app", "save_spark"),
    ("Here's an idea: voice notes for walks", "save_spark"),
    ("I had a dream about flying", "dream.log"),
    ("Write down this dream", "dream.log"),
    ("Show my past dreams", "dream.search"),
    ("Dream journal", "dream.search"),
    ("Made a sale £40", "work.log_sales"),
    ("Knocked off at 5", "work.set_hours"),
    ("How much did I sell?", "work.get_stats"),
    ("Sales this week", "work.get_stats"),
]


# ---------------------------------------------------------------------------
# Extraction goldens — payload contracts (not every paraphrase).
# ---------------------------------------------------------------------------

def _titles(payload: dict[str, Any]) -> list[str]:
    return [t["title"].lower() for t in payload.get("tasks", [])]


def _check_free_time_2h(payload: dict[str, Any]) -> None:
    assert payload["duration_minutes"] == 120
    assert payload["end"] > payload["start"]


def _check_plan_day_activities(payload: dict[str, Any]) -> None:
    assert payload["apply"] is True
    titles = _titles(payload)
    assert any("tennis" in t for t in titles)
    assert any("bath" in t for t in titles)
    assert any("cook" in t for t in titles)


def _check_plan_day_sunday_sparks(payload: dict[str, Any]) -> None:
    assert payload["apply"] is False
    titles = _titles(payload)
    assert sum(1 for t in titles if "spark" in t) == 2
    assert any("gym" in t for t in titles)
    assert any("cook" in t or "dinner" in t for t in titles)


def _check_schedule_week_report(payload: dict[str, Any]) -> None:
    assert payload["duration_minutes"] == 120
    title = payload["title"].lower()
    assert "report" in title or "design" in title
    assert payload["deadline"] > int(time.time() * 1000) + 3_600_000


def _check_schedule_count_climbing(payload: dict[str, Any]) -> None:
    assert payload["count"] == 3
    assert payload["prefer_spread"] is True
    assert payload["apply"] is False
    assert "climb" in payload["title"].lower()


def _check_schedule_count_yoga(payload: dict[str, Any]) -> None:
    assert payload["count"] == 2
    assert payload["prefer_spread"] is True
    assert "yoga" in payload["title"].lower()


def _check_block_coding(payload: dict[str, Any]) -> None:
    assert payload["duration_minutes"] == 180
    assert "cod" in payload["title"].lower()


def _check_chained_study_research(payload: dict[str, Any]) -> None:
    assert "events" in payload
    events = payload["events"]
    assert len(events) == 2
    assert events[0]["title"].lower() == "study"
    assert "research" in events[1]["title"].lower()
    assert events[0]["end_time"] - events[0]["start_time"] == 3 * 60 * 60 * 1000
    assert events[1]["end_time"] - events[1]["start_time"] == 2 * 60 * 60 * 1000
    assert events[1]["start_time"] == events[0]["end_time"]
    assert datetime.fromtimestamp(events[0]["start_time"] / 1000).hour == 11


def _check_create_meeting(payload: dict[str, Any]) -> None:
    assert "Meeting" in payload["title"] or "meeting" in payload["title"].lower()
    assert payload["end_time"] > payload["start_time"]


def _check_personal_code_event(payload: dict[str, Any]) -> None:
    assert "cod" in payload["title"].lower()
    assert payload["category"] == "personal"
    assert payload["end_time"] > payload["start_time"]


def _check_lunch_midday(payload: dict[str, Any]) -> None:
    assert payload["title"].lower() == "lunch"
    assert payload["category"] == "personal"
    start = datetime.fromtimestamp(payload["start_time"] / 1000)
    end = datetime.fromtimestamp(payload["end_time"] / 1000)
    assert start.hour == 12 and start.minute == 0
    assert end.hour == 13 and end.minute == 0


EXTRACTION_CASES: list[tuple[str, str, PayloadCheck]] = [
    ("When am I free tomorrow for 2 hours?", "calendar.find_free_time", _check_free_time_2h),
    (
        "Plan my day for tomorrow, tennis, bath and cooking dinner",
        "calendar.plan_day",
        _check_plan_day_activities,
    ),
    (
        "I want to do two of my sparks, go gym and cook dinner on Sunday",
        "calendar.plan_day",
        _check_plan_day_sunday_sparks,
    ),
    (
        "Finish the design report this week, 2 hours",
        "calendar.schedule_task",
        _check_schedule_week_report,
    ),
    (
        "I want to go climbing 3 times this week",
        "calendar.schedule_task",
        _check_schedule_count_climbing,
    ),
    (
        "Make time for yoga twice this week",
        "calendar.schedule_task",
        _check_schedule_count_yoga,
    ),
    ("block 3 hours for coding", "calendar.block_time", _check_block_coding),
    (
        "make a event called study for 3 hours at 11:00 tomorrow "
        "followed by a research block for 2 hours",
        "calendar.create_event",
        _check_chained_study_research,
    ),
    (
        "Schedule a meeting tomorrow at 3pm",
        "calendar.create_event",
        _check_create_meeting,
    ),
    (
        "make a personal event tomorrow to code at 6pm-7pm",
        "calendar.create_event",
        _check_personal_code_event,
    ),
    (
        "Add a lunch break around midday.",
        "calendar.create_event",
        _check_lunch_midday,
    ),
]


# ---------------------------------------------------------------------------
# Negatives — must skip or never route to these tools.
# ---------------------------------------------------------------------------

NEGATIVE_SKIP: list[str] = [
    "How are you today?",
    "thanks",
    "what do you think?",
]

# Heuristic must not claim coder.run (code stays on MLX path).
NEGATIVE_NOT_TOOL: list[tuple[str, str]] = [
    ("refactor the login module and fix the bug", "coder.run"),
]


def _assert_routes(msg: str, tool: str) -> None:
    plan = try_fast_heuristic_plan(msg)
    assert plan is not None, f"expected tool for {msg!r}"
    assert plan.tool == tool, f"{msg!r} -> {plan.tool!r}, expected {tool!r}"
    assert plan.respond_mode == "passthrough", f"{msg!r} respond_mode={plan.respond_mode!r}"


def test_routing_by_intent():
    for msg, tool in ROUTING_CASES:
        _assert_routes(msg, tool)


def test_extraction_contracts():
    for msg, tool, check in EXTRACTION_CASES:
        plan = try_fast_heuristic_plan(msg)
        assert plan is not None, msg
        assert plan.tool == tool, f"{msg!r} -> {plan.tool!r}, expected {tool!r}"
        payload = json.loads(plan.tool_input or "{}")
        check(payload)


def test_negative_skips_chat():
    for msg in NEGATIVE_SKIP:
        assert try_fast_heuristic_plan(msg) is None, msg


def test_negative_not_coder():
    for msg, banned in NEGATIVE_NOT_TOOL:
        plan = try_fast_heuristic_plan(msg)
        if plan is not None:
            assert plan.tool != banned, f"{msg!r} incorrectly routed to {banned}"


def test_parse_plan_coerces_object_tool_input():
    raw = json.dumps(
        {
            "intent": "tool_use",
            "tool": "calendar.create_event",
            "tool_input": {
                "title": "Dentist",
                "start_time": 1,
                "end_time": 2,
            },
            "reasoning": "schedule",
            "response": None,
        }
    )
    plan = parse_plan(raw, "add dentist tomorrow")
    assert plan.intent == "tool_use"
    assert plan.tool == "calendar.create_event"
    assert isinstance(plan.tool_input, str)
    assert json.loads(plan.tool_input)["title"] == "Dentist"
    assert plan.respond_mode == "passthrough"


def test_parse_plan_empty_falls_back_to_heuristic():
    plan = parse_plan("", "What's on my calendar today?")
    assert plan.tool == "calendar.get_today"
    assert plan.respond_mode == "passthrough"


def test_respond_mode_defaults():
    assert default_respond_mode("coder.run") == "llm"
    assert default_respond_mode("calendar.create_event") == "passthrough"
    assert default_respond_mode("echo") == "passthrough"


if __name__ == "__main__":
    test_routing_by_intent()
    test_extraction_contracts()
    test_negative_skips_chat()
    test_negative_not_coder()
    test_parse_plan_coerces_object_tool_input()
    test_parse_plan_empty_falls_back_to_heuristic()
    test_respond_mode_defaults()
    print(f"ok ({len(ROUTING_CASES)} routing, {len(EXTRACTION_CASES)} extraction)")
