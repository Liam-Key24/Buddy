"""Memory maintenance tools."""

from prompts.domains.base import DomainSpec

MEMORY = DomainSpec(
    id="memory",
    prefixes=("memory.",),
    classify_rules=(
        "/handover or conversation handover → memory.handover.",
        "/maintain or memory maintenance → memory.maintain.",
    ),
    fill_by_tool={
        "memory.handover": ('{"conversation_id":"..."} optional if inferable from context.',),
        "memory.maintain": ('{"conversation_id":"..."} optional.',),
    },
    respond_hints=("Handover and maintain outputs are user-facing — keep confirmation brief.",),
)
