from pydantic import BaseModel


class HistoryMessage(BaseModel):
    role: str
    content: str


class MemoryContextPayload(BaseModel):
    handover: str | None = None
    working: str | None = None
    project: str | None = None
    preferences: str | None = None
    decisions: str | None = None
    errors: str | None = None
    tools: str | None = None
    reflections: str | None = None
    workspace: str | None = None
    learned_patterns: str | None = None
    stale_sparks: str | None = None


def format_history(history: list) -> list[dict]:
    formatted = []
    for msg in history:
        role = msg.role if hasattr(msg, "role") else msg["role"]
        content = msg.content if hasattr(msg, "content") else msg["content"]
        if role in ("user", "assistant"):
            formatted.append({"role": role, "content": content})
    return formatted


def build_memory_block(memory: MemoryContextPayload | None) -> str:
    if memory is None:
        return ""
    sections = []
    if memory.handover:
        sections.append(f"## Project Handover\n{memory.handover}")
    if memory.workspace:
        sections.append(f"## Workspace\n{memory.workspace}")
    if memory.working:
        sections.append(f"## Active Task\n{memory.working}")
    if memory.learned_patterns:
        sections.append(f"## Learned Patterns\n{memory.learned_patterns}")
    if memory.project:
        sections.append(f"## Project Knowledge\n{memory.project}")
    if memory.preferences:
        sections.append(f"## User Preferences\n{memory.preferences}")
    if memory.decisions:
        sections.append(f"## Decisions\n{memory.decisions}")
    if memory.errors:
        sections.append(f"## Known Errors\n{memory.errors}")
    if memory.tools:
        sections.append(f"## Recent Tools\n{memory.tools}")
    if memory.reflections:
        sections.append(f"## Reflections\n{memory.reflections}")
    if memory.stale_sparks:
        sections.append(
            f"## Stale Sparks (need attention)\n{memory.stale_sparks}\n"
            "Gently nudge the user about these when relevant. Use update_spark to archive, respark, or delete when they decide."
        )
    if not sections:
        return ""
    return "Relevant project memory:\n\n" + "\n\n".join(sections)


def build_messages(
    memory: MemoryContextPayload | None,
    history: list,
    message: str,
) -> list[dict]:
    messages = format_history(history)
    memory_block = build_memory_block(memory)
    user_content = message
    if memory_block:
        user_content = f"{memory_block}\n\n---\n\nUser message: {message}"
    messages.append({"role": "user", "content": user_content})
    return messages
