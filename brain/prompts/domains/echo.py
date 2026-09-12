"""Echo tool (tests / debug)."""

from prompts.domains.base import DomainSpec

ECHO = DomainSpec(
    id="echo",
    prefixes=(),
    exact_tools=("echo",),
    classify_rules=('User says "echo <text>" → echo.',),
    fill_by_tool={
        "echo": ("tool_input is the text to echo (string or {\"text\":\"...\"}).",),
    },
)
