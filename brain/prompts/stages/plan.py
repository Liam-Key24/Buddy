"""Backward-compatible single-shot planner (tests / fallback)."""

from __future__ import annotations

from prompts.core import (
    CLASSIFY_ONLY_RULES,
    CORE_IDENTITY,
    FALLBACK_RULES,
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
from prompts.registry import collect_classify_rules


def build(ctx: PromptContext) -> str:
    tools_block = tool_catalog_block(ctx.available_tools)
    now_iso, now_ms = time_block()
    domain_rules = collect_classify_rules(ctx)

    rules = bullet_lines(
        [
            *UNIVERSAL_PLAN_RULES,
            *FILL_RULES[:2],  # fill + omit unknowns
            *domain_rules,
            *FALLBACK_RULES,
            "Output JSON only.",
        ]
    )

    return join_sections(
        f"{CORE_IDENTITY} You are the planning module. Analyze the user's message.",
        JSON_ONLY_RULE,
        f"Available tools:\n{tools_block}",
        f"Current local time: {now_iso}",
        f"Current unix time (milliseconds): {now_ms}",
        f"Output schema:\n{{\n{PLAN_OUTPUT_WITH_TOOL_INPUT}\n}}",
        f"Rules:\n{rules}",
    )
