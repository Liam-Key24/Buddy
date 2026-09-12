"""Buddy prompt system — modular, extensible planner and responder prompts.

Public API (stable):
  build_classify_system_prompt, build_fill_system_prompt,
  build_continue_system_prompt, build_plan_system_prompt,
  respond_system_prompt, CHAT_SYSTEM_PROMPT, RESPOND_SYSTEM_PROMPT

Extend capabilities by adding a DomainSpec in prompts/domains/ and registering
it in prompts/domains/__init__.py.
"""

from __future__ import annotations

from prompts.core import PromptContext
from prompts.stages import agent_loop, classify, fill, plan, respond
from prompts.stages.respond import (
    CHAT_SYSTEM_PROMPT,
    RESPOND_BASE,
    build_tool_narration,
    respond_system_prompt,
)

# Backward-compatible alias
RESPOND_SYSTEM_PROMPT = build_tool_narration()


def build_classify_system_prompt(available_tools: str = "") -> str:
    """Stage 1: pick intent + tool only. Never invent tool_input."""
    return classify.build(PromptContext.from_tools(available_tools))


def build_fill_system_prompt(available_tools: str = "", tool: str = "") -> str:
    """Stage 2: fill tool_input for a chosen tool; omit unknowns."""
    return fill.build(PromptContext.from_tools(available_tools, tool))


def build_continue_system_prompt(available_tools: str = "") -> str:
    """Agent-loop step: ask | tool | finish given scratchpad."""
    return agent_loop.build(PromptContext.from_tools(available_tools))


def build_plan_system_prompt(available_tools: str = "") -> str:
    """Backward-compatible single-shot planner (tests / fallback). Prefer classify+fill."""
    return plan.build(PromptContext.from_tools(available_tools))


__all__ = [
    "PromptContext",
    "CHAT_SYSTEM_PROMPT",
    "RESPOND_SYSTEM_PROMPT",
    "RESPOND_BASE",
    "build_classify_system_prompt",
    "build_fill_system_prompt",
    "build_continue_system_prompt",
    "build_plan_system_prompt",
    "respond_system_prompt",
    "build_tool_narration",
]
