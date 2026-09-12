"""Stage 1: intent + tool selection (no tool_input)."""

from __future__ import annotations

from prompts.core import (
    CLASSIFY_ONLY_RULES,
    CORE_IDENTITY,
    FALLBACK_RULES,
    JSON_ONLY_RULE,
    PLAN_OUTPUT_FIELDS,
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
            *CLASSIFY_ONLY_RULES,
            *FALLBACK_RULES,
            *domain_rules,
            "Output JSON only — no tool_input field.",
        ]
    )

    return join_sections(
        f"{CORE_IDENTITY} You are the intent classifier.",
        JSON_ONLY_RULE,
        f"Available tools:\n{tools_block}",
        f"Current local time: {now_iso} ({now_ms} ms).",
        f"Output schema:\n{{\n{PLAN_OUTPUT_FIELDS}\n}}",
        f"Rules:\n{rules}",
    )
