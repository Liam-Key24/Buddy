"""Domain registry: filter rules by available tools."""

from __future__ import annotations

from prompts.core import PromptContext, parse_tool_names
from prompts.domains import ALL_DOMAINS, DomainSpec


def active_domains(ctx: PromptContext) -> list[DomainSpec]:
    names = parse_tool_names(ctx.available_tools)
    if not names:
        return list(ALL_DOMAINS)
    return [d for d in ALL_DOMAINS if d.matches_catalog(names)]


def collect_classify_rules(ctx: PromptContext) -> tuple[str, ...]:
    rules: list[str] = []
    for domain in active_domains(ctx):
        rules.extend(domain.classify_rules)
    return tuple(rules)


def collect_fill_hints(ctx: PromptContext, tool: str) -> tuple[str, ...]:
    if not tool:
        return ()
    for domain in active_domains(ctx):
        if domain.matches_tool(tool):
            hints = domain.fill_hints_for(tool)
            if hints:
                return hints
    return ()


def collect_continue_rules(ctx: PromptContext) -> tuple[str, ...]:
    rules: list[str] = []
    for domain in active_domains(ctx):
        rules.extend(domain.continue_rules)
    return tuple(rules)


def collect_respond_hints(tool_name: str | None) -> tuple[str, ...]:
    if not tool_name:
        return ()
    hints: list[str] = []
    for domain in ALL_DOMAINS:
        if domain.matches_tool(tool_name):
            hints.extend(domain.respond_hints)
    return tuple(hints)
