"""Code agent tool."""

from prompts.domains.base import DomainSpec

CODER = DomainSpec(
    id="coder",
    prefixes=(),
    exact_tools=("coder.run",),
    classify_rules=(
        "Coding / implementing / debugging in a workspace → coder.run.",
        "File edits on disk → matching fs tool, not coder.run, unless they want agent exploration.",
        "Set focus from user tone: planning|asking|debugging|focused.",
    ),
    fill_by_tool={
        "coder.run": (
            '{"prompt": "<user request>", "focus": "planning|asking|debugging|focused"}.',
            "Pass the full user request as prompt; do not truncate.",
        ),
    },
    respond_hints=(
        "When coder.run was invoked, acknowledge the task was sent to the Code Agent briefly.",
    ),
)
