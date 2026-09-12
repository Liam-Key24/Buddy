"""Natural-language response after chat or tool execution."""

from __future__ import annotations

from prompts.core import CORE_IDENTITY, bullet_lines, join_sections
from prompts.registry import collect_respond_hints

CHAT_SYSTEM_PROMPT = join_sections(
    CORE_IDENTITY,
    "Answer clearly and directly. This turn is conversation — no tools are attached.",
    "If they later ask you to change their calendar, documents, todos, fitness, money, or socials, you can do that in the app; do not pretend you already did.",
    "Be concise unless they ask for depth. Use short paragraphs and markdown bullets for lists of times, tasks, or options — never one dense paragraph.",
    "Reply immediately — no reasoning tags, no JSON.",
)

RESPOND_BASE = join_sections(
    CORE_IDENTITY,
    "The user invoked a tool and you received the result.",
    "Write a brief, friendly natural language response incorporating the tool result. Be concise.",
    "Format with markdown: bold labels and bullet lists for times, sparks, tasks, and proposals. Do not mash several findings into one paragraph.",
)

GLOBAL_RESPOND_HINTS: tuple[str, ...] = (
    "If the tool result already contains a complete user-facing message, lightly trim — do not duplicate long JSON.",
    "If the tool failed or returned an error, explain plainly and suggest what the user can try next.",
    "Never invent data not present in the tool result.",
)


def build_chat() -> str:
    return CHAT_SYSTEM_PROMPT


def build_tool_narration(tool_name: str | None = None) -> str:
    if tool_name:
        domain_hints = collect_respond_hints(tool_name)
    else:
        # No tool context — include all domain narration guidance (legacy behavior).
        from prompts.domains import ALL_DOMAINS

        domain_hints = []
        seen: set[str] = set()
        for domain in ALL_DOMAINS:
            for hint in domain.respond_hints:
                if hint not in seen:
                    seen.add(hint)
                    domain_hints.append(hint)
    hints = bullet_lines([*GLOBAL_RESPOND_HINTS, *domain_hints])
    return join_sections(RESPOND_BASE, f"Guidance:\n{hints}")


def respond_system_prompt(*, has_tool_result: bool, tool_name: str | None = None) -> str:
    if has_tool_result:
        return build_tool_narration(tool_name)
    return build_chat()
