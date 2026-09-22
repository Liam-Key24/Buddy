"""Application-owned mutation policy. The model does not commit destructive writes."""

from __future__ import annotations

from typing import Any, Literal

Policy = Literal["commit", "preview", "clarify"]

COMMIT_KINDS = {
    "goal_create",
    "goal_update",
    "spark_capture",
    "spark_dismiss",
    "spark_promote",
    "propose_sessions",
    "approve_proposals",
    "reject_proposals",
    "chat",
}

PREVIEW_KINDS = {
    "calendar_delete",
    "calendar_move",
    "calendar_update",
    "pause_others",
}


def classify_operation(
    *,
    kind: str,
    target_count: int = 0,
    all_matching: bool = False,
    has_session_id: bool = False,
    confidence: float = 1.0,
) -> Policy:
    if kind == "session_outcome":
        if has_session_id and target_count <= 1:
            return "commit"
        if target_count == 0:
            return "clarify"
        return "preview"
    if kind in PREVIEW_KINDS:
        if kind.startswith("calendar_") and target_count == 0:
            return "clarify"
        if (
            kind.startswith("calendar_")
            and target_count > 1
            and not all_matching
            and not has_session_id
        ):
            return "clarify"
        if confidence < 0.45:
            return "preview"
        return "preview"
    if kind in COMMIT_KINDS:
        return "commit"
    if kind.startswith("calendar_"):
        return "preview" if target_count else "clarify"
    return "commit"


def preview_payload(
    *,
    kind: str,
    records: list[dict[str, Any]],
    why: str,
) -> dict[str, Any]:
    return {
        "kind": kind,
        "count": len(records),
        "records": records,
        "why": why,
        "actions": ["approve", "adjust", "cancel"],
    }
