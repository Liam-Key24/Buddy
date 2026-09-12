"""Email and git tools."""

from prompts.domains.base import DomainSpec

EXTERNAL = DomainSpec(
    id="external",
    prefixes=(),
    exact_tools=("send_email", "git_push"),
    classify_rules=(
        "Email draft → send_email (drafted for approval, not sent automatically).",
        "Git push request → git_push (requires user approval).",
    ),
    fill_by_tool={
        "send_email": (
            '{"to":"...","subject":"...","body":"..."} name optional.',
            "Omit unknown recipient — Clarification asks.",
        ),
        "git_push": ('{"remote?":"...","branch?":"...","repo_path?":"..."}.',),
    },
    respond_hints=(
        "Confirm email was drafted for review, not sent.",
        "Confirm git push was queued for approval.",
    ),
)
