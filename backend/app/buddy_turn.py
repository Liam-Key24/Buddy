"""Normalize Cloud AI JSON into a valid BuddyTurn before persistence."""

from __future__ import annotations

import uuid
from typing import Any

from .schemas import OPERATION_KINDS, BuddyTurn, CalendarAction, GoalUpdate

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
    operations, op_errors = _normalize_operations(data)
    errors.extend(op_errors)
    data["operations"] = operations
    data["validation_errors"] = errors
    if operations and int(data.get("schema_version") or 1) < 2:
        data["schema_version"] = 2
    else:
        data.setdefault("schema_version", 1)

    return data


def parse_buddy_turn(raw: dict[str, Any]) -> BuddyTurn:
    return BuddyTurn.model_validate(normalize_buddy_turn_dict(raw))


OPERATION_KIND_ALIASES = {
    "create_goal": "goal_create",
    "set_goal": "goal_create",
    "new_goal": "goal_create",
    "add_goal": "goal_create",
    "update_goal": "goal_update",
    "revise_goal": "goal_update",
    "edit_goal": "goal_update",
    "pause": "pause_others",
    "pause_other_goals": "pause_others",
    "propose": "propose_sessions",
    "propose_session": "propose_sessions",
    "schedule": "propose_sessions",
    "approve": "approve_proposals",
    "reject": "reject_proposals",
    "capture_spark": "spark_capture",
    "dismiss_spark": "spark_dismiss",
    "promote_spark": "spark_promote",
    "delete": "calendar_delete",
    "delete_session": "calendar_delete",
    "remove_session": "calendar_delete",
    "update_session": "calendar_update",
    "move_session": "calendar_move",
    "reschedule": "calendar_move",
    "complete": "session_outcome",
    "missed": "session_outcome",
}


def _normalize_operations(data: dict[str, Any]) -> tuple[list[dict[str, Any]], list[dict[str, str]]]:
    errors: list[dict[str, str]] = []
    raw_ops = data.get("operations")
    if isinstance(raw_ops, dict):
        raw_ops = [raw_ops]
    if isinstance(raw_ops, list) and raw_ops:
        cleaned = []
        for item in raw_ops:
            if not isinstance(item, dict):
                continue
            op, err = _clean_operation(item, default_confidence=float(data.get("confidence") or 0.5))
            if err:
                errors.append(err)
            if op:
                cleaned.append(op)
        if cleaned:
            return cleaned, errors
    return _legacy_operations(data), errors


def _clean_operation(
    item: dict[str, Any], *, default_confidence: float
) -> tuple[dict[str, Any] | None, dict[str, str] | None]:
    kind_raw = str(item.get("kind") or item.get("type") or item.get("op") or "").strip().lower().replace(" ", "_")
    kind = OPERATION_KIND_ALIASES.get(kind_raw, kind_raw)
    if kind not in OPERATION_KINDS:
        return None, {"path": "operations[].kind", "error": f"unknown operation {kind_raw or 'missing'}"}
    payload = item.get("payload")
    if not isinstance(payload, dict):
        payload = {k: v for k, v in item.items() if k not in {
            "id", "kind", "type", "op", "target_type", "target_id", "target_ref",
            "payload", "assumptions", "confidence", "disposition",
        }}
    assumptions = item.get("assumptions") or []
    if isinstance(assumptions, str):
        assumptions = [assumptions]
    try:
        confidence = float(item.get("confidence", default_confidence))
    except (TypeError, ValueError):
        confidence = default_confidence
    disposition = item.get("disposition") or item.get("decision")
    if disposition in {"can_commit", "commit"}:
        disposition = "commit"
    elif disposition in {"needs_clarification", "clarify"}:
        disposition = "clarify"
    elif disposition in {"needs_approval", "preview", "approve"}:
        disposition = "needs_approval"
    elif disposition not in {"commit", "clarify", "needs_approval"}:
        disposition = None
    target_id = item.get("target_id") or payload.get("goal_id") or payload.get("session_id") or payload.get("spark_id")
    target_type = item.get("target_type")
    if not target_type:
        if kind.startswith("goal_") or kind in {"propose_sessions", "pause_others"}:
            target_type = "goal"
        elif kind.startswith("calendar_") or kind == "session_outcome":
            target_type = "session"
        elif kind.startswith("spark_"):
            target_type = "spark"
        elif kind in {"approve_proposals", "reject_proposals"}:
            target_type = "batch"
    return {
        "id": str(item.get("id") or uuid.uuid4()),
        "kind": kind,
        "target_type": target_type,
        "target_id": str(target_id) if target_id else None,
        "target_ref": item.get("target_ref"),
        "payload": payload,
        "assumptions": [str(a) for a in assumptions if a],
        "confidence": confidence,
        "disposition": disposition,
    }, None


def _legacy_operations(data: dict[str, Any]) -> list[dict[str, Any]]:
    """One-time expansion of pre-v2 BuddyTurn fields. Apply path never reads these fields."""
    ops: list[dict[str, Any]] = []
    create_refs: list[str] = []
    updates = data.get("goal_updates") or []
    if isinstance(updates, dict):
        updates = [updates]
    for i, update in enumerate(updates):
        if not isinstance(update, dict):
            continue
        action = str(update.get("action") or "update").lower()
        if action == "create":
            kind = "goal_create"
        elif action == "pause_others":
            kind = "pause_others"
        else:
            kind = "goal_update"
        ref = f"goal_{i+1}"
        if kind == "goal_create":
            create_refs.append(ref)
        ops.append(
            {
                "id": str(uuid.uuid4()),
                "kind": kind,
                "target_type": "goal",
                "target_id": None,
                "target_ref": ref,
                "payload": {k: v for k, v in update.items() if k != "action"},
                "assumptions": [],
                "confidence": float(data.get("confidence") or 0.5),
                "disposition": None,
            }
        )

    ra = data.get("requested_action") or {}
    if not isinstance(ra, dict):
        ra = {}
    intents = data.get("intents") or []
    spark_content = ra.get("spark_content")
    spark_id = ra.get("spark_id")
    if "spark_capture" in intents and spark_content:
        ops.append(_op("spark_capture", payload={"spark_content": spark_content}, target_type="spark"))
    if "spark_dismiss" in intents and spark_id:
        ops.append(_op("spark_dismiss", payload={"spark_id": spark_id}, target_id=spark_id, target_type="spark"))
    if "spark_promote" in intents and spark_id:
        ops.append(_op("spark_promote", payload={"spark_id": spark_id}, target_id=spark_id, target_type="spark"))

    action_type = ra.get("type") or "none"
    if action_type == "propose_sessions":
        refs = create_refs or [None]
        for ref in refs:
            ops.append(_op("propose_sessions", payload=dict(ra), target_ref=ref, target_type="goal", target_id=ra.get("batch_id")))
    elif action_type == "approve_proposals":
        ops.append(_op("approve_proposals", payload=dict(ra), target_id=ra.get("batch_id"), target_type="batch"))
    elif action_type == "reject_proposals":
        ops.append(_op("reject_proposals", payload=dict(ra), target_id=ra.get("batch_id"), target_type="batch"))

    if "session_outcome" in intents and ra.get("session_id") and ra.get("outcome"):
        ops.append(
            _op(
                "session_outcome",
                payload={"session_id": ra.get("session_id"), "outcome": ra.get("outcome")},
                target_id=ra.get("session_id"),
                target_type="session",
            )
        )

    for cal_action in data.get("calendar_actions") or []:
        if not isinstance(cal_action, dict):
            continue
        op_name = cal_action.get("op")
        kind = "session_outcome" if op_name == "mark_outcome" else f"calendar_{op_name}"
        ops.append(
            _op(
                kind,
                payload=dict(cal_action),
                target_id=cal_action.get("session_id"),
                target_type="session",
            )
        )
    return ops


def _op(
    kind: str,
    *,
    payload: dict[str, Any] | None = None,
    target_id: str | None = None,
    target_ref: str | None = None,
    target_type: str | None = None,
) -> dict[str, Any]:
    return {
        "id": str(uuid.uuid4()),
        "kind": kind,
        "target_type": target_type,
        "target_id": target_id,
        "target_ref": target_ref,
        "payload": payload or {},
        "assumptions": [],
        "confidence": 0.5,
        "disposition": None,
    }


def operation_goal_update(op: dict[str, Any]) -> GoalUpdate:
    payload = op.get("payload") or {}
    action = "create" if op.get("kind") == "goal_create" else "pause_others" if op.get("kind") == "pause_others" else "update"
    facts = payload.get("facts") if isinstance(payload.get("facts"), dict) else {}
    return GoalUpdate(
        action=action,
        title=payload.get("title"),
        domain=payload.get("domain"),
        target=payload.get("target"),
        deadline=payload.get("deadline"),
        baseline=payload.get("baseline"),
        frequency=payload.get("frequency"),
        commitment=payload.get("commitment"),
        status=payload.get("status"),
        facts=facts,
    )


def operation_calendar_action(op: dict[str, Any]) -> CalendarAction | None:
    payload = op.get("payload") or {}
    kind = op.get("kind") or ""
    op_name = payload.get("op")
    if not op_name:
        if kind == "session_outcome":
            op_name = "mark_outcome"
        elif kind.startswith("calendar_"):
            op_name = kind.replace("calendar_", "", 1)
    if not op_name:
        return None
    session_id = payload.get("session_id") or (op.get("target_id") if op.get("target_type") == "session" else None)
    try:
        return CalendarAction.model_validate(
            {
                "op": op_name,
                "session_id": session_id,
                "title_contains": payload.get("title_contains") or payload.get("match_title"),
                "date": payload.get("date"),
                "goal_id": payload.get("goal_id") or (op.get("target_id") if op.get("target_type") == "goal" else None),
                "statuses": payload.get("statuses") or [],
                "all_matching": bool(payload.get("all_matching", False)),
                "new_title": payload.get("new_title"),
                "new_start_at": payload.get("new_start_at") or payload.get("start_at"),
                "new_end_at": payload.get("new_end_at") or payload.get("end_at"),
                "outcome": payload.get("outcome"),
                "notes": payload.get("notes"),
            }
        )
    except Exception:
        return None


def operations_from_turn(turn: BuddyTurn) -> list[dict[str, Any]]:
    """Return the typed operation list. Legacy fields are expanded at parse time."""
    return [op.model_dump() for op in turn.operations]
