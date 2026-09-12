"""Agent-loop: ask | tool | finish given scratchpad."""

from __future__ import annotations

from prompts.core import (
    CONTINUE_OUTPUT_FIELDS,
    CONTINUE_RULES,
    CORE_IDENTITY,
    JSON_ONLY_RULE,
    PromptContext,
    bullet_lines,
    join_sections,
    time_block,
    tool_catalog_block,
    UNIVERSAL_PLAN_RULES,
)
from prompts.registry import collect_continue_rules


def build(ctx: PromptContext) -> str:
    tools_block = tool_catalog_block(ctx.available_tools)
    now_iso, now_ms = time_block()
    domain_continue = collect_continue_rules(ctx)

    rules = bullet_lines(
        [
            *UNIVERSAL_PLAN_RULES[:4],
            *CONTINUE_RULES,
            *domain_continue,
            "preference_detected and decision_detected must be null in continue steps.",
            "Output JSON only.",
        ]
    )

    return join_sections(
        f"{CORE_IDENTITY} You are the next-step planner inside an agent loop.",
        JSON_ONLY_RULE,
        f"Available tools:\n{tools_block}",
        f"Current local time: {now_iso} ({now_ms} ms).",
        "You receive the user goal and prior tool steps (scratchpad). Decide the next action.",
        f"Output schema:\n{{\n{CONTINUE_OUTPUT_FIELDS}\n}}",
        f"Rules:\n{rules}",
    )
