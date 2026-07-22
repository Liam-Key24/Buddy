"""Quick checks for calendar chat planning helpers."""

import json

from parser import (
    default_respond_mode,
    parse_plan,
    try_fast_heuristic_plan,
)


def test_object_tool_input_is_coerced_to_string():
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


def test_heuristic_create_event():
    plan = parse_plan("", "Schedule a meeting tomorrow at 3pm")
    assert plan.intent == "tool_use"
    assert plan.tool == "calendar.create_event"
    payload = json.loads(plan.tool_input)
    assert "Meeting" in payload["title"] or "meeting" in payload["title"].lower()
    assert payload["end_time"] > payload["start_time"]
    assert plan.respond_mode == "passthrough"


def test_heuristic_today():
    plan = parse_plan("", "What's on my calendar today?")
    assert plan.tool == "calendar.get_today"
    assert plan.respond_mode == "passthrough"


def test_fast_heuristic_skips_chat():
    assert try_fast_heuristic_plan("How are you today?") is None


def test_fast_heuristic_calendar():
    plan = try_fast_heuristic_plan("What's on today?")
    assert plan is not None
    assert plan.tool == "calendar.get_today"
    assert plan.respond_mode == "passthrough"


def test_fast_heuristic_skips_coder():
    # Code stays on MLX plan path even if heuristics would match.
    plan = try_fast_heuristic_plan("refactor the login module and fix the bug")
    if plan is not None:
        assert plan.tool != "coder.run"


def test_coder_respond_mode_is_llm():
    assert default_respond_mode("coder.run") == "llm"
    assert default_respond_mode("calendar.create_event") == "passthrough"
    assert default_respond_mode("echo") == "passthrough"


if __name__ == "__main__":
    test_object_tool_input_is_coerced_to_string()
    test_heuristic_create_event()
    test_heuristic_today()
    test_fast_heuristic_skips_chat()
    test_fast_heuristic_calendar()
    test_fast_heuristic_skips_coder()
    test_coder_respond_mode_is_llm()
    print("ok")
