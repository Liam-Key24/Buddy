"""Shared prompt primitives: context, schemas, and assembly helpers."""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
import re
from typing import Iterable


@dataclass(frozen=True)
class PromptContext:
    """Runtime context for prompt assembly. All fields are optional extensions."""

    available_tools: str = ""
    tool: str = ""
    workspace_hint: str = ""
    surface: str = ""  # e.g. calendar, todo, fitness, chat, code_agent

    @classmethod
    def from_tools(cls, available_tools: str = "", tool: str = "") -> PromptContext:
        return cls(available_tools=available_tools, tool=tool.strip())

    def with_tool(self, tool: str) -> PromptContext:
        return PromptContext(
            available_tools=self.available_tools,
            tool=tool.strip(),
            workspace_hint=self.workspace_hint,
            surface=self.surface,
        )


def parse_tool_names(available_tools: str) -> set[str]:
    """Extract tool names from the planner catalog (`- name: description`)."""
    names: set[str] = set()
    for line in (available_tools or "").splitlines():
        line = line.strip()
        if not line.startswith("-"):
            continue
        body = line[1:].strip()
        if not body:
            continue
        # `calendar.look: ...` or `calendar.look — ...`
        name = re.split(r"[\s:—–-]", body, maxsplit=1)[0].strip()
        if name:
            names.add(name)
    return names


def tool_catalog_block(available_tools: str) -> str:
    return available_tools.strip() or "- (no tools registered)"


def time_block() -> tuple[str, int]:
    now = datetime.now().astimezone()
    return now.isoformat(timespec="seconds"), int(now.timestamp() * 1000)


def join_sections(*parts: str | None, sep: str = "\n\n") -> str:
    return sep.join(p.strip() for p in parts if p and p.strip())


def bullet_lines(lines: Iterable[str]) -> str:
    return "\n".join(f"- {line.strip()}" for line in lines if line and line.strip())


# --- Plan JSON schema fragments (shared across stages) ---

PLAN_OUTPUT_FIELDS = """
  "intent": "chat" | "tool_use",
  "tool": "<tool name from Available tools>" | null,
  "reasoning": "<brief>",
  "response": "<direct reply if intent is chat, else null>",
  "task_state": "started" | "updated" | "completed" | null,
  "mode_hint": "planning" | "asking" | "debugging" | "focused" | null,
  "preference_detected": { "key": "<string>", "value": "<string>", "confidence": 0.0-1.0, "source": "explicit" | "inferred" } | null,
  "decision_detected": { "decision": "<string>", "reason": "<string>" } | null,
  "next": "ask" | "tool" | "finish" | null"""


PLAN_OUTPUT_WITH_TOOL_INPUT = PLAN_OUTPUT_FIELDS.replace(
    '"tool": "<tool name from Available tools>" | null,',
    '"tool": "<tool name from Available tools>" | null,\n  "tool_input": {} | "<json string>" | null,',
)

CONTINUE_OUTPUT_FIELDS = """
  "intent": "chat" | "tool_use",
  "tool": "<tool name>" | null,
  "tool_input": {} | "<json string>" | null,
  "reasoning": "<brief>",
  "response": "<final user reply if next is finish, else null>",
  "task_state": "started" | "updated" | "completed" | null,
  "next": "ask" | "tool" | "finish",
  "preference_detected": null,
  "decision_detected": null"""


# --- Universal rules (every planning stage) ---

CORE_IDENTITY = "You are Buddy, a helpful local AI assistant running on the user's Mac."

JSON_ONLY_RULE = "Output ONLY valid JSON (no markdown fences, no prose outside JSON)."

UNIVERSAL_PLAN_RULES: tuple[str, ...] = (
    "Re-decide fresh every message from text, history, memory, and workspace — do not assume prior turns chose the right tool.",
    "Clear desire to act → intent tool_use with the best registered tool. NEVER chat-promise (\"I can help you…\", \"I'll look into that…\") when a tool should run.",
    "Missing details (duration, time, path, recipient, id) still mean tool_use — Clarification will ask. Do NOT invent args; omit unknown required fields.",
    "If the user message bundles several actions (food + calendar + money + spark), pick the most urgent concrete tool for this turn; the agent loop or a follow-up can handle the rest.",
    "Pure conversation, opinions, or ambiguous intent with no clear action → intent chat, next finish, provide response.",
    "When unsure between chat and a weak tool match, prefer one short clarifying question in response (intent chat, next ask) over guessing tool args.",
    "Only choose tools listed under Available tools. Never invent tool names.",
    "preference_detected only for explicit user prefs (confidence >= 0.9). decision_detected for clear commitments.",
)

CLASSIFY_ONLY_RULES: tuple[str, ...] = (
    "Classify intent and tool only — omit tool_input entirely.",
    "For tool_use set next to tool (or omit). For chat set next to finish.",
)

FILL_RULES: tuple[str, ...] = (
    "Fill only fields the user (or memory context) clearly provided.",
    "OMIT unknown required fields so Clarification can ask — never invent durations, times, paths, recipients, or ids.",
    'Convert relative times to unix milliseconds when the user gave a clock or clear day.',
    "intent must be tool_use; next must be tool; response must be null.",
)

CONTINUE_RULES: tuple[str, ...] = (
    'next "tool": call another tool (search then update, read then edit, look then pin, etc.). Fill tool_input; omit unknowns.',
    'next "ask": need a user choice not covered by tool schemas — put the question in response; intent chat.',
    'next "finish": goal done or blocked; put the user-facing summary in response; intent chat.',
    "Never chat-promise a tool you should call — use next tool.",
    "Do not repeat a tool that already succeeded with the same args.",
    "Prefer finish after a successful write/list unless the user goal clearly needs another step.",
    "After a successful read/search, follow with update/patch/pin only when the user asked to change something.",
)

FALLBACK_RULES: tuple[str, ...] = (
    "If no tool fits, respond helpfully in chat — do not force a tool.",
    "If memory lists Active Sparks and the user wants spark work, prefer calendar.organize with spark_id when known.",
    "Disk paths (~/…) → fs tools. In-app Documents → docs.*. Work sales/hours → work.*. Personal ledger → money.*.",
)
