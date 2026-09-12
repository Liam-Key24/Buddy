"""Prompt system modularity and backward compatibility."""

from prompts import (
    CHAT_SYSTEM_PROMPT,
    build_classify_system_prompt,
    build_continue_system_prompt,
    build_fill_system_prompt,
    build_plan_system_prompt,
    respond_system_prompt,
)
from prompts.core import parse_tool_names
from prompts.registry import active_domains, collect_classify_rules, collect_respond_hints
from prompts.core import PromptContext


CALENDAR_TOOLS = "- calendar.look: agenda\n- calendar.organize: schedule"
LIFE_TOOLS = (
    "- fitness.log_food: log meal\n- money.log: expense\n"
    "- todo.add: task\n- calendar.look: agenda"
)


def test_public_api_builds_non_empty():
    assert "intent classifier" in build_classify_system_prompt(CALENDAR_TOOLS).lower()
    assert "calendar.organize" in build_fill_system_prompt(CALENDAR_TOOLS, "calendar.organize")
    assert "scratchpad" in build_continue_system_prompt(CALENDAR_TOOLS).lower()
    assert "planning module" in build_plan_system_prompt(CALENDAR_TOOLS).lower()


def test_domain_rules_filter_by_catalog():
    ctx = PromptContext.from_tools(CALENDAR_TOOLS)
    rules = collect_classify_rules(ctx)
    assert any("calendar.organize" in r for r in rules)
    assert not any("fitness.log_food" in r for r in rules)

    life_ctx = PromptContext.from_tools(LIFE_TOOLS)
    life_rules = collect_classify_rules(life_ctx)
    assert any("fitness" in r.lower() for r in life_rules)
    assert any("money" in r.lower() for r in life_rules)


def test_active_domains_empty_catalog_includes_all():
    ctx = PromptContext.from_tools("")
    assert len(active_domains(ctx)) >= 8


def test_parse_tool_names():
    names = parse_tool_names("- calendar.look: x\n- save_spark: y")
    assert names == {"calendar.look", "save_spark"}


def test_respond_tool_specific_hints():
    chat = respond_system_prompt(has_tool_result=False)
    tool = respond_system_prompt(has_tool_result=True, tool_name="calendar.look")
    assert chat == CHAT_SYSTEM_PROMPT
    assert "free slots" in tool.lower() or "calendar" in tool.lower()
    assert collect_respond_hints("save_spark")


def test_classify_omits_tool_input_instruction():
    prompt = build_classify_system_prompt(CALENDAR_TOOLS)
    assert "no tool_input" in prompt.lower()


def test_fill_locks_tool_name():
    prompt = build_fill_system_prompt(CALENDAR_TOOLS, "calendar.pin")
    assert '"tool": "calendar.pin"' in prompt


if __name__ == "__main__":
    test_public_api_builds_non_empty()
    test_domain_rules_filter_by_catalog()
    test_active_domains_empty_catalog_includes_all()
    test_parse_tool_names()
    test_respond_tool_specific_hints()
    test_classify_omits_tool_input_instruction()
    test_fill_locks_tool_name()
    print("ok prompts")
