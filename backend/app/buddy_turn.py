"""Normalize Cloud AI JSON into a valid BuddyTurn before persistence."""

from __future__ import annotations

from typing import Any

from .schemas import BuddyTurn

INTENT_ALIASES = {
    "set_goal": "goal_create",
    "create_goal": "goal_create",
    "new_goal": "goal_create",
    "add_goal": "goal_create",
    "update_goal": "goal_update",
    "revise_goal": "goal_update",
    "edit_goal": "goal_update",
    "progress": "goal_progress",
    "check_progress": "goal_progress",
    "request_plan": "goal_plan_request",
    "plan": "goal_plan_request",
    "schedule": "goal_plan_request",
    "propose": "goal_plan_request",
    "propose_sessions": "goal_plan_request",
    "approve": "calendar_proposal_decision",
    "reject": "calendar_proposal_decision",
    "decision": "calendar_proposal_decision",
    "complete": "session_outcome",
    "missed": "session_outcome",
    "outcome": "session_outcome",
    "spark": "spark_capture",
    "capture_spark": "spark_capture",
    "promote": "spark_promote",
    "dismiss": "spark_dismiss",
}

ACTION_TYPE_ALIASES = {
    "propose": "propose_sessions",
    "propose_session": "propose_sessions",
    "schedule": "propose_sessions",
    "approve": "approve_proposals",
    "confirm": "approve_proposals",
    "reject": "reject_proposals",
    "confirm_goal_creation": "none",
    "none": "none",
    "noop": "none",
}

VALID_INTENTS = {
    "chat",
    "goal_create",
    "goal_update",
    "goal_progress",
    "goal_plan_request",
    "calendar_proposal_decision",
    "session_outcome",
    "spark_capture",
    "spark_promote",
    "spark_dismiss",
}


def normalize_buddy_turn_dict(raw: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise ValueError("BuddyTurn must be an object")
    if "assistant_text" not in raw:
        raise ValueError("BuddyTurn missing assistant_text")
    data = dict(raw)
    if not isinstance(data.get("assistant_text"), str) or not data["assistant_text"].strip():
        data["assistant_text"] = "I'm here — tell me a bit more and we'll shape a plan you control."

    intents_in = data.get("intents") or ["chat"]
    if isinstance(intents_in, str):
        intents_in = [intents_in]
    intents: list[str] = []
    for item in intents_in:
        key = str(item).strip().lower().replace(" ", "_")
        mapped = INTENT_ALIASES.get(key, key)
        if mapped in VALID_INTENTS and mapped not in intents:
            intents.append(mapped)
    if not intents:
        intents = ["chat"]
    data["intents"] = intents

    updates = data.get("goal_updates") or []
    if isinstance(updates, dict):
        updates = [updates]
    clean_updates = []
    for u in updates:
        if not isinstance(u, dict):
            continue
        action = str(u.get("action") or "update").lower()
        if action in {"create", "new", "set"}:
            action = "create"
        elif action in {"pause", "pause_others", "switch"}:
            action = "pause_others"
        else:
            action = "update"
        facts = u.get("facts") if isinstance(u.get("facts"), dict) else {}
        clean_updates.append(
            {
                "action": action,
                "title": u.get("title"),
                "domain": u.get("domain"),
                "target": u.get("target"),
                "deadline": u.get("deadline"),
                "baseline": u.get("baseline"),
                "frequency": u.get("frequency"),
                "commitment": u.get("commitment"),
                "status": u.get("status"),
                "facts": facts,
            }
        )
    data["goal_updates"] = clean_updates

    ra = data.get("requested_action")
    if ra is None or ra == "":
        data["requested_action"] = {"type": "none"}
    elif isinstance(ra, str):
        mapped = ACTION_TYPE_ALIASES.get(ra.strip().lower().replace(" ", "_"), "none")
        data["requested_action"] = {"type": mapped}
    elif isinstance(ra, dict):
        t = str(ra.get("type") or "none").lower().replace(" ", "_")
        t = ACTION_TYPE_ALIASES.get(t, t)
        if t not in {"propose_sessions", "approve_proposals", "reject_proposals", "none"}:
            t = "none"
        data["requested_action"] = {
            "type": t,
            "batch_id": ra.get("batch_id"),
            "session_id": ra.get("session_id"),
            "outcome": ra.get("outcome"),
            "spark_id": ra.get("spark_id"),
            "spark_content": ra.get("spark_content"),
        }
    else:
        data["requested_action"] = {"type": "none"}

    try:
        data["confidence"] = float(data.get("confidence", 0.5))
    except (TypeError, ValueError):
        data["confidence"] = 0.5

    if data.get("clarification") is not None and not isinstance(data.get("clarification"), str):
        data["clarification"] = str(data["clarification"])

    return data


def parse_buddy_turn(raw: dict[str, Any]) -> BuddyTurn:
    return BuddyTurn.model_validate(normalize_buddy_turn_dict(raw))
