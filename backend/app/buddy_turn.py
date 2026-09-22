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
    "delete_session": "calendar_delete",
    "delete_sessions": "calendar_delete",
    "remove_session": "calendar_delete",
    "cancel_session": "calendar_delete",
    "edit_session": "calendar_update",
    "update_session": "calendar_update",
    "rename_session": "calendar_update",
    "move_session": "calendar_move",
    "reschedule": "calendar_move",
    "reschedule_session": "calendar_move",
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
    "calendar_delete",
    "calendar_update",
    "calendar_move",
}

CALENDAR_OP_ALIASES = {
    "delete": "delete",
    "remove": "delete",
    "cancel": "delete",
    "update": "update",
    "edit": "update",
    "rename": "update",
    "move": "move",
    "reschedule": "move",
    "mark_outcome": "mark_outcome",
    "complete": "mark_outcome",
    "completed": "mark_outcome",
    "missed": "mark_outcome",
}


def normalize_buddy_turn_dict(raw: dict[str, Any]) -> dict[str, Any]:
    if not isinstance(raw, dict):
        raise ValueError("BuddyTurn must be an object")
    if "assistant_text" not in raw:
        raise ValueError("BuddyTurn missing assistant_text")
    data = dict(raw)
    if not isinstance(data.get("assistant_text"), str) or not data["assistant_text"].strip():
        data["assistant_text"] = "I'm here — tell me a bit more and we'll shape a plan you control."

    errors: list[dict[str, str]] = list(data.get("validation_errors") or [])
    intents_in = data.get("intents") or ["chat"]
    if isinstance(intents_in, str):
        intents_in = [intents_in]
    intents: list[str] = []
    for item in intents_in:
        key = str(item).strip().lower().replace(" ", "_")
        mapped = INTENT_ALIASES.get(key, key)
        if mapped in VALID_INTENTS:
            if mapped not in intents:
                intents.append(mapped)
        else:
            errors.append({"path": "intents", "error": f"unknown intent {key}"})
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
        key = ra.strip().lower().replace(" ", "_")
        mapped = ACTION_TYPE_ALIASES.get(key, key)
        if mapped not in {"propose_sessions", "approve_proposals", "reject_proposals", "none"}:
            errors.append({"path": "requested_action.type", "error": f"unknown action {mapped}"})
        data["requested_action"] = {"type": mapped}
    elif isinstance(ra, dict):
        t = str(ra.get("type") or "none").lower().replace(" ", "_")
        t = ACTION_TYPE_ALIASES.get(t, t)
        if t not in {"propose_sessions", "approve_proposals", "reject_proposals", "none"}:
            errors.append({"path": "requested_action.type", "error": f"unknown action {t}"})
        data["requested_action"] = {
            "type": t,
            "batch_id": ra.get("batch_id"),
            "session_id": ra.get("session_id"),
            "outcome": ra.get("outcome"),
            "spark_id": ra.get("spark_id"),
            "spark_content": ra.get("spark_content"),
        }
    else:
        errors.append({"path": "requested_action", "error": "requested_action must be an object"})
        data["requested_action"] = {"type": "none"}

    try:
        data["confidence"] = float(data.get("confidence", 0.5))
    except (TypeError, ValueError):
        data["confidence"] = 0.5

    if data.get("clarification") is not None and not isinstance(data.get("clarification"), str):
        data["clarification"] = str(data["clarification"])

    raw_qs = data.get("clarification_questions") or []
    if isinstance(raw_qs, dict):
        raw_qs = [raw_qs]
    clean_qs = []
    for i, q in enumerate(raw_qs[:4]):
        if not isinstance(q, dict):
            continue
        label = q.get("label") or q.get("question")
        if not label:
            continue
        at = str(q.get("answer_type") or "short_text").lower().replace(" ", "_")
        if at not in {
            "short_text",
            "number",
            "date",
            "time",
            "single_choice",
            "multiple_choice",
            "yes_no",
        }:
            at = "short_text"
        opts = q.get("options") or []
        if not isinstance(opts, list):
            opts = []
        clean_qs.append(
            {
                "id": str(q.get("id") or f"q{i+1}"),
                "label": str(label),
                "help_text": q.get("help_text"),
                "answer_type": at,
                "required": bool(q.get("required", True)),
                "options": [str(o) for o in opts],
                "suggested_answer": q.get("suggested_answer"),
                "reason": q.get("reason"),
            }
        )
    data["clarification_questions"] = clean_qs

    raw_actions = data.get("calendar_actions") or []
    if isinstance(raw_actions, dict):
        raw_actions = [raw_actions]
    clean_actions = []
    for item in raw_actions:
        if not isinstance(item, dict):
            continue
        op_raw = str(item.get("op") or item.get("action") or item.get("type") or "").lower().replace(" ", "_")
        op = CALENDAR_OP_ALIASES.get(op_raw)
        if not op:
            errors.append({"path": "calendar_actions[].op", "error": f"unknown calendar op {op_raw or 'missing'}"})
            continue
        statuses = item.get("statuses") or item.get("status_in") or []
        if isinstance(statuses, str):
            statuses = [statuses]
        match_title = item.get("title_contains") or item.get("match_title")
        if not match_title and op != "update":
            match_title = item.get("title")
        clean_actions.append(
            {
                "op": op,
                "session_id": item.get("session_id"),
                "title_contains": match_title,
                "date": item.get("date"),
                "goal_id": item.get("goal_id"),
                "statuses": [str(s) for s in statuses if s],
                "all_matching": bool(item.get("all_matching", False)),
                "new_title": item.get("new_title"),
                "new_start_at": item.get("new_start_at") or item.get("start_at"),
                "new_end_at": item.get("new_end_at") or item.get("end_at"),
                "outcome": item.get("outcome"),
                "notes": item.get("notes"),
            }
        )
    data["calendar_actions"] = clean_actions
    data["validation_errors"] = errors
    data.setdefault("schema_version", 1)

    return data


def parse_buddy_turn(raw: dict[str, Any]) -> BuddyTurn:
    return BuddyTurn.model_validate(normalize_buddy_turn_dict(raw))


def operations_from_turn(turn: BuddyTurn) -> list[dict[str, Any]]:
    """Map today's BuddyTurn fields into ordered operations (temporary adapter)."""
    ops: list[dict[str, Any]] = []
    for update in turn.goal_updates:
        if update.action == "create":
            kind = "goal_create"
        elif update.action == "pause_others":
            kind = "pause_others"
        else:
            kind = "goal_update"
        ops.append({"kind": kind, "goal_update": update})

    action = turn.requested_action
    spark_content = action.spark_content if action else None
    spark_id = action.spark_id if action else None
    if "spark_capture" in turn.intents and spark_content:
        ops.append({"kind": "spark_capture", "spark_content": spark_content})
    if "spark_dismiss" in turn.intents and spark_id:
        ops.append({"kind": "spark_dismiss", "spark_id": spark_id})
    if "spark_promote" in turn.intents and spark_id:
        ops.append({"kind": "spark_promote", "spark_id": spark_id})

    if action and action.type == "propose_sessions":
        ops.append({"kind": "propose_sessions", "requested_action": action})
    elif action and action.type == "approve_proposals":
        ops.append({"kind": "approve_proposals", "requested_action": action})
    elif action and action.type == "reject_proposals":
        ops.append({"kind": "reject_proposals", "requested_action": action})

    if "session_outcome" in turn.intents and action and action.session_id and action.outcome:
        ops.append({"kind": "session_outcome", "requested_action": action})

    for cal_action in turn.calendar_actions:
        kind = "session_outcome" if cal_action.op == "mark_outcome" else f"calendar_{cal_action.op}"
        ops.append({"kind": kind, "calendar_action": cal_action})
    return ops
