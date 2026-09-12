"""Stage 2: fill tool_input for a chosen tool."""

from __future__ import annotations

from prompts.core import (
    CORE_IDENTITY,
    FILL_RULES,
    JSON_ONLY_RULE,
    PLAN_OUTPUT_WITH_TOOL_INPUT,
    PromptContext,
    bullet_lines,
    join_sections,
    time_block,
    tool_catalog_block,
    UNIVERSAL_PLAN_RULES,
)
from prompts.registry import collect_fill_hints


def build(ctx: PromptContext) -> str:
    tool = ctx.tool or "(tool from request)"
    tools_block = tool_catalog_block(ctx.available_tools)
    now_iso, now_ms = time_block()
    tool_hints = collect_fill_hints(ctx, tool)

    rules = bullet_lines(
        [
            *UNIVERSAL_PLAN_RULES[:3],  # act, omit unknowns, no chat-promise
            *FILL_RULES,
            *tool_hints,
            "preference_detected only for explicit prefs (confidence >= 0.9), e.g. key preferred_activity_duration value \"120\".",
            "Output JSON only.",
        ]
    )

    schema = PLAN_OUTPUT_WITH_TOOL_INPUT.replace(
        '"tool": "<tool name from Available tools>" | null,',
        f'"tool": "{tool}",',
    ).replace('"next": "ask" | "tool" | "finish" | null', '"next": "tool"')

    return join_sections(
        f"{CORE_IDENTITY} You are the argument filler. The tool is already chosen: `{tool}`.",
        JSON_ONLY_RULE,
        f"Available tools (for shape reference):\n{tools_block}",
        f"Current local time: {now_iso} ({now_ms} ms).",
        f"Output schema:\n{{\n{schema}\n}}",
        f"Rules:\n{rules}",
    )
