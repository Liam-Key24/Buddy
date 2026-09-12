"""Heuristic calendar / lifestyle routing goldens.

Structure:
  - ROUTING: message → tool (intent coverage; add paraphrases as data)
  - EXTRACTION: payload shape for a few goldens per capability
  - NEGATIVE: chat / coder must not be stolen into calendar tools
  - CONTRACT: parse_plan / respond_mode plumbing
"""

from __future__ import annotations

import json
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
    ("What's on today?", "calendar.look"),
    ("When am I working?", "calendar.look"),
    ("What's happening today?", "calendar.look"),
    ("What's on my calendar today?", "calendar.look"),
    ("Show me tomorrow", "calendar.look"),
    ("Do I have anything this week?", "calendar.look"),
    ("next week plans?", "calendar.look"),
    ("What's planned next week?", "calendar.look"),
    ("Show me Friday", "calendar.look"),
    # free time
    ("When am I free tomorrow for 2 hours?", "calendar.look"),
    ("Got any free time tomorrow?", "calendar.look"),
    ("Any open slots today?", "calendar.look"),
    # organize day / week
    ("Plan my day for tomorrow, tennis, bath and cooking dinner", "calendar.organize"),
    ("Organise my day for tomorrow, tennis and bath", "calendar.organize"),
    ("Map out my day: gym and grocery run", "calendar.organize"),
    ("I want to do two sparks and gym on Sunday", "calendar.organize"),
    (
        "I want to do two of my sparks, go gym and cook dinner on Sunday",
        "calendar.organize",
    ),
    # capacity
    ("What's my capacity today?", "calendar.look"),
    ("How busy is my day?", "calendar.look"),
    ("Am I overloaded today?", "calendar.look"),
    # pin clock events
    ("Schedule a meeting tomorrow at 3pm", "calendar.pin"),
    ("Pencil in a meeting tomorrow at 3pm", "calendar.pin"),
    ("Set up an appointment for Friday at 10am", "calendar.pin"),
    ("I have dentist tomorrow at 2pm", "calendar.pin"),
    (
        "make a event called study for 3 hours at 11:00 tomorrow "
        "followed by a research block for 2 hours",
        "calendar.pin",
    ),
    ("make a personal event tomorrow to code at 6pm-7pm", "calendar.pin"),
    ("Add a lunch break around midday.", "calendar.pin"),
    ("Add a lunch break tomorrow around midday.", "calendar.pin"),
    # organize focus / multi
    ("block 3 hours for coding", "calendar.organize"),
    ("can you block 2 hours for coding", "calendar.organize"),
    ("could you block off time for deep work", "calendar.organize"),
    ("please hold 90 minutes for writing", "calendar.organize"),
    ("reserve 1 hour for focus", "calendar.organize"),
    ("set aside time for studying", "calendar.organize"),
    ("Finish the design report this week, 2 hours", "calendar.organize"),
    ("Find time for the design report", "calendar.organize"),
    ("Need to finish the report by Friday", "calendar.organize"),
    ("I want to go climbing 3 times this week", "calendar.organize"),
    ("Make time for yoga twice this week", "calendar.organize"),
    (
        "lets plan my week after work i want it to include 3 30min study sessions, "
        "2 climbing sessions for 1:30hr and on the weekend 2 3hour study sessions for cyber secruity",
        "calendar.organize",
    ),
    ("plan my week with climbing and study", "calendar.organize"),
    (
        "lets plan my week next week to include 2 climbing sessions 2 gym sessions after work",
        "calendar.organize",
    ),
    ("two climbing sessions and two gym sessions next week after work", "calendar.organize"),
    # mutate / search
    ("Get rid of the meeting from my calendar", "calendar.pin"),
    ("Take it off my calendar", "calendar.pin"),
    ("Where is gym on my calendar?", "calendar.look"),
    ("Do I have a meeting on my calendar?", "calendar.look"),
    # sparks / lifestyle (high-precision only on fast path)
    ("Note to self: camping app idea", "save_spark"),
    ("spark: voice notes for walks", "save_spark"),
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

def _items(payload: dict[str, Any]) -> list[dict[str, Any]]:
    return [i for i in (payload.get("items") or []) if isinstance(i, dict)]


def _item_titles(payload: dict[str, Any]) -> list[str]:
    return [t["title"].lower() for t in _items(payload)]


def _check_free_time_2h(payload: dict[str, Any]) -> None:
    assert payload.get("focus") == "free"
    assert payload.get("when") == "tomorrow"
    assert payload["duration_minutes"] == 120


def _check_next_week_plans(payload: dict[str, Any]) -> None:
    assert payload.get("focus") == "events"
    assert payload.get("when") == "next_week"


def _check_next_week_free(payload: dict[str, Any]) -> None:
    assert payload.get("focus") == "free"
    assert payload.get("when") == "next_week"


def _check_plan_day_activities(payload: dict[str, Any]) -> None:
    assert payload.get("mode") == "propose"
    titles = _item_titles(payload)
    assert any("tennis" in t for t in titles)
    assert any("bath" in t for t in titles)
    assert any("cook" in t for t in titles)
    assert all("duration_minutes" not in t for t in _items(payload))


def _check_plan_day_sunday_sparks(payload: dict[str, Any]) -> None:
    assert payload.get("mode") == "propose"
    titles = _item_titles(payload)
    assert sum(1 for t in titles if "spark" in t) == 2
    assert any("gym" in t for t in titles)
    assert any("cook" in t or "dinner" in t for t in titles)
    assert all("duration_minutes" not in t for t in _items(payload))


def _check_schedule_week_report(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert items
    title = items[0]["title"].lower()
    assert "report" in title or "design" in title
    assert items[0]["duration_minutes"] == 120


def _check_schedule_count_climbing(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert items
    assert items[0].get("count") == 3
    assert "climb" in items[0]["title"].lower()
    assert "duration_minutes" not in items[0]
    assert payload.get("mode") == "propose"


def _check_next_week_climb_gym(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert payload.get("mode") == "propose"
    assert payload.get("window") == "next_week", payload
    assert "after_work" in (payload.get("constraints") or []), payload
    climb = next((t for t in items if "climb" in t["title"].lower()), None)
    gym = next((t for t in items if t["title"].lower() == "gym"), None)
    assert climb is not None and gym is not None, payload
    assert climb.get("count") == 2, payload
    assert gym.get("count") == 2, payload
    assert "duration_minutes" not in climb, payload
    assert "duration_minutes" not in gym, payload
    assert climb.get("when") == "after_work" or "after_work" in (
        payload.get("constraints") or []
    )


def _check_week_with_climb_study(payload: dict[str, Any]) -> None:
    titles = {t["title"].lower() for t in _items(payload)}
    assert any("climb" in t for t in titles), payload
    assert any("study" in t for t in titles), payload


def _check_schedule_count_yoga(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert items
    assert items[0].get("count") == 2
    assert "yoga" in items[0]["title"].lower()
    assert "duration_minutes" not in items[0]


def _check_schedule_week_multi(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert len(items) >= 3, payload
    assert payload.get("mode") == "propose"
    assert "after_work" in (payload.get("constraints") or [])
    study = next((t for t in items if t["title"].lower() == "study"), None)
    climb = next((t for t in items if "climb" in t["title"].lower()), None)
    cyber = next((t for t in items if "cyber" in t["title"].lower()), None)
    assert study is not None, payload
    assert study["duration_minutes"] == 30 and study["count"] == 3
    assert climb is not None, payload
    assert climb["duration_minutes"] == 90 and climb["count"] == 2
    assert cyber is not None, payload
    assert cyber["duration_minutes"] == 180 and cyber["count"] == 2
    assert cyber.get("when") == "weekend"


def _check_free_time_no_duration(payload: dict[str, Any]) -> None:
    assert payload.get("focus") == "free"
    assert payload.get("duration_minutes") == 60


def _check_block_coding(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert items
    assert items[0]["duration_minutes"] == 180
    assert "cod" in items[0]["title"].lower()
    assert payload.get("mode") == "propose"


def _check_block_no_duration(payload: dict[str, Any]) -> None:
    items = _items(payload)
    assert items
    assert items[0].get("title")
    assert "duration_minutes" not in items[0]


def _check_chained_study_research(payload: dict[str, Any]) -> None:
    assert "study" in payload.get("title", "").lower()
    assert payload.get("start")
    start = int(payload["start"])
    assert datetime.fromtimestamp(start / 1000).hour == 11


def _check_create_meeting(payload: dict[str, Any]) -> None:
    assert "Meeting" in payload["title"] or "meeting" in payload["title"].lower()
    assert payload.get("start")


def _check_personal_code_event(payload: dict[str, Any]) -> None:
    assert "cod" in payload["title"].lower()
    assert payload.get("category") == "personal"
    assert payload.get("start")


def _check_lunch_midday(payload: dict[str, Any]) -> None:
    assert payload["title"].lower() == "lunch"
    assert payload.get("category") == "personal"
    start = datetime.fromtimestamp(int(payload["start"]) / 1000)
    assert start.hour == 12 and start.minute == 0


EXTRACTION_CASES: list[tuple[str, str, PayloadCheck]] = [
    ("next week plans?", "calendar.look", _check_next_week_plans),
    ("What's planned next week?", "calendar.look", _check_next_week_plans),
    (
        "give me free slots next week not today",
        "calendar.look",
        _check_next_week_free,
    ),
    ("give me all week next week", "calendar.look", _check_next_week_plans),
    ("When am I free tomorrow for 2 hours?", "calendar.look", _check_free_time_2h),
    ("Got any free time tomorrow?", "calendar.look", _check_free_time_no_duration),
    ("Any open slots today?", "calendar.look", _check_free_time_no_duration),
    (
        "Plan my day for tomorrow, tennis, bath and cooking dinner",
        "calendar.organize",
        _check_plan_day_activities,
    ),
    (
        "I want to do two of my sparks, go gym and cook dinner on Sunday",
        "calendar.organize",
        _check_plan_day_sunday_sparks,
    ),
    (
        "Finish the design report this week, 2 hours",
        "calendar.organize",
        _check_schedule_week_report,
    ),
    (
        "I want to go climbing 3 times this week",
        "calendar.organize",
        _check_schedule_count_climbing,
    ),
    (
        "Make time for yoga twice this week",
        "calendar.organize",
        _check_schedule_count_yoga,
    ),
    (
        "lets plan my week after work i want it to include 3 30min study sessions, "
        "2 climbing sessions for 1:30hr and on the weekend 2 3hour study sessions for cyber secruity",
        "calendar.organize",
        _check_schedule_week_multi,
    ),
    (
        "lets plan my week next week to include 2 climbing sessions 2 gym sessions after work",
        "calendar.organize",
        _check_next_week_climb_gym,
    ),
    (
        "two climbing sessions and two gym sessions next week after work",
        "calendar.organize",
        _check_next_week_climb_gym,
    ),
    (
        "plan my week with climbing and study",
        "calendar.organize",
        _check_week_with_climb_study,
    ),
    ("block 3 hours for coding", "calendar.organize", _check_block_coding),
    ("set aside time for studying", "calendar.organize", _check_block_no_duration),
    (
        "make a event called study for 3 hours at 11:00 tomorrow "
        "followed by a research block for 2 hours",
        "calendar.pin",
        _check_chained_study_research,
    ),
    (
        "Schedule a meeting tomorrow at 3pm",
        "calendar.pin",
        _check_create_meeting,
    ),
    (
        "make a personal event tomorrow to code at 6pm-7pm",
        "calendar.pin",
        _check_personal_code_event,
    ),
    (
        "Add a lunch break around midday.",
        "calendar.pin",
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
    assert plan.tool == "calendar.pin"
    assert isinstance(plan.tool_input, str)
    assert json.loads(plan.tool_input)["title"] == "Dentist"
    assert plan.respond_mode == "passthrough"


def test_parse_plan_empty_falls_back_to_heuristic():
    plan = parse_plan("", "What's on my calendar today?")
    assert plan.tool == "calendar.look"
    assert plan.respond_mode == "passthrough"


def test_respond_mode_defaults():
    assert default_respond_mode("coder.run") == "llm"
    assert default_respond_mode("calendar.create_event") == "passthrough"
    assert default_respond_mode("echo") == "passthrough"


def test_soft_idea_language_skips_fast_path():
    for msg in (
        "What if we planted fruit trees",
        "Thinking about a Scotland trip",
        "Maybe we could build a deck",
    ):
        assert try_fast_heuristic_plan(msg) is None, msg


def test_precise_spark_phrases_fast_path():
    plan = try_fast_heuristic_plan("Note to self: buy compost bins")
    assert plan is not None
    assert plan.tool == "save_spark"


def test_soft_book_climbing_schedules_without_duration():
    for msg in (
        "hey i want to book some time this week for climbing",
        "i want to book climbing today",
    ):
        plan = try_fast_heuristic_plan(msg)
        assert plan is not None, msg
        assert plan.tool == "calendar.organize", msg
        payload = json.loads(plan.tool_input or "{}")
        items = payload.get("items") or []
        assert items and "Climb" in items[0].get("title", ""), payload
        assert "duration_minutes" not in items[0], payload


def test_repair_schedule_task_title_when_mlx_omits():
    from parser import PlanResponse, repair_schedule_task_title

    plan = PlanResponse(
        intent="tool_use",
        tool="calendar.organize",
        tool_input='{"mode": "propose", "items": []}',
        reasoning="fill",
        response=None,
    )
    fixed = repair_schedule_task_title(plan, "can we book climbing next week")
    payload = json.loads(fixed.tool_input or "{}")
    items = payload.get("items") or []
    assert items and "Climb" in items[0].get("title", ""), payload


def test_repair_find_free_time_collapses_zero_width_range():
    from parser import PlanResponse, repair_find_free_time_fill

    now = 1785600918866
    plan = PlanResponse(
        intent="tool_use",
        tool="calendar.look",
        tool_input=json.dumps(
            {"duration_minutes": 60, "start": now, "end": now, "limit": 5}
        ),
        reasoning="fill",
        response=None,
    )
    fixed = repair_find_free_time_fill(plan, "what times am i free next week?")
    payload = json.loads(fixed.tool_input or "{}")
    assert payload["end"] > payload["start"], payload
    assert payload["end"] - payload["start"] >= 86_400_000, payload


def test_plan_my_week_routes_and_fills_tasks():
    from parser import repair_classified_tool, repair_schedule_task_title, PlanResponse

    msg = (
        "lets plan my week after work i want it to include 3 30min study sessions, "
        "2 climbing sessions for 1:30hr and on the weekend 2 3hour study sessions for cyber secruity"
    )
    intent, tool = repair_classified_tool("chat", None, msg, [])
    assert intent == "tool_use" and tool == "calendar.organize"

    plan = try_fast_heuristic_plan(msg)
    assert plan is not None
    assert plan.tool == "calendar.organize"
    _check_schedule_week_multi(json.loads(plan.tool_input or "{}"))

    thin = PlanResponse(
        intent="tool_use",
        tool="calendar.organize",
        tool_input='{"mode": "propose", "items": [{"title": "Study"}]}',
        reasoning="fill",
        response=None,
    )
    fixed = repair_schedule_task_title(thin, msg)
    _check_schedule_week_multi(json.loads(fixed.tool_input or "{}"))


def test_repair_classified_plan_my_week_from_chat():
    from parser import repair_classified_tool

    intent, tool = repair_classified_tool(
        "chat", None, "plan my week with climbing and study after work", []
    )
    assert intent == "tool_use"
    assert tool == "calendar.organize"


if __name__ == "__main__":
    test_routing_by_intent()
    test_extraction_contracts()
    test_negative_skips_chat()
    test_negative_not_coder()
    test_parse_plan_coerces_object_tool_input()
    test_parse_plan_empty_falls_back_to_heuristic()
    test_respond_mode_defaults()
    test_soft_idea_language_skips_fast_path()
    test_precise_spark_phrases_fast_path()
    test_soft_book_climbing_schedules_without_duration()
    test_repair_schedule_task_title_when_mlx_omits()
    test_repair_find_free_time_collapses_zero_width_range()
    test_plan_my_week_routes_and_fills_tasks()
    test_repair_classified_plan_my_week_from_chat()
    print(f"ok ({len(ROUTING_CASES)} routing, {len(EXTRACTION_CASES)} extraction)")
