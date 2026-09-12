"""Classify → repair tool name → fill pipeline for /chat/plan."""

from __future__ import annotations

import json
import logging
from typing import Any, Optional

from parser import (
    PlanResponse,
    heuristic_classify_tool,
    heuristic_fill_for_tool,
    normalize_plan,
    parse_plan,
    repair_classified_tool,
    repair_find_free_time_fill,
    repair_schedule_task_title,
    try_fast_heuristic_plan,
)
from prompts import (
    build_classify_system_prompt,
    build_continue_system_prompt,
    build_fill_system_prompt,
)

logger = logging.getLogger("buddy.brain.pipeline")


def _history_for_repair(history: list) -> list:
    out = []
    for item in history or []:
        if hasattr(item, "role"):
            out.append({"role": item.role, "content": item.content})
        elif isinstance(item, dict):
            out.append(item)
    return out


def _strip_tool_input(plan: PlanResponse) -> PlanResponse:
    """Classify stage must not carry invented args into repair."""
    plan.tool_input = None
    return plan


def run_classify_fill(
    *,
    message: str,
    history: list,
    memory_messages: list,
    available_tools: str,
    mlx_complete,
    scratchpad: Optional[list[dict[str, Any]]] = None,
    allow_mlx: bool = True,
) -> PlanResponse:
    """Full plan path: continue-step OR classify→repair→fill.

    `mlx_complete(system, messages) -> str` — inject MLX or a test double.
    Confident heuristics skip MLX. `allow_mlx=False` is probe-only (no generate).
    """
    if scratchpad:
        if not allow_mlx:
            return normalize_plan(
                PlanResponse(
                    intent="chat",
                    reasoning="heuristic_only miss",
                    response=None,
                    next="finish",
                )
            )
        return _continue_step(
            message=message,
            history=history,
            memory_messages=memory_messages,
            available_tools=available_tools,
            mlx_complete=mlx_complete,
            scratchpad=scratchpad,
        )

    fast = try_fast_heuristic_plan(message)
    if fast is not None:
        logger.info("fast-path tool=%s", fast.tool)
        return fast
    if not allow_mlx:
        return normalize_plan(
            PlanResponse(
                intent="chat",
                reasoning="heuristic_only miss",
                response=None,
                next="finish",
            )
        )

    classified = _classify(
        message=message,
        memory_messages=memory_messages,
        available_tools=available_tools,
        mlx_complete=mlx_complete,
    )
    intent, tool = repair_classified_tool(
        classified.intent,
        classified.tool,
        message,
        _history_for_repair(history),
    )
    classified.intent = intent
    classified.tool = tool
    classified = normalize_plan(classified)

    if classified.intent != "tool_use" or not classified.tool:
        classified.next = "finish"
        return normalize_plan(classified)

    filled = _fill(
        message=message,
        tool=classified.tool,
        memory_messages=memory_messages,
        available_tools=available_tools,
        mlx_complete=mlx_complete,
        base=classified,
    )
    filled.next = "tool"
    filled = normalize_plan(filled)
    filled = repair_schedule_task_title(filled, message)
    filled = repair_find_free_time_fill(filled, message)
    return filled


def _classify(
    *,
    message: str,
    memory_messages: list,
    available_tools: str,
    mlx_complete,
) -> PlanResponse:
    try:
        raw = mlx_complete(
            build_classify_system_prompt(available_tools),
            memory_messages,
        )
        plan = parse_plan(raw, message)
        plan = _strip_tool_input(plan)
        # Drop any model-invented tool_input from a confused classify call.
        if plan.intent == "tool_use" and plan.tool:
            plan.tool_input = None
        return normalize_plan(plan)
    except Exception as e:
        logger.warning("classify mlx failed, heuristic tool repair: %s", e)
        tool = heuristic_classify_tool(message)
        if tool:
            return normalize_plan(
                PlanResponse(
                    intent="tool_use",
                    tool=tool,
                    tool_input=None,
                    reasoning=f"Classify repair after MLX error ({type(e).__name__}).",
                    response=None,
                    next="tool",
                )
            )
        return normalize_plan(
            PlanResponse(
                intent="chat",
                tool=None,
                tool_input=None,
                reasoning=f"Classify fallback ({type(e).__name__}).",
                response="I'm here to help.",
                next="finish",
            )
        )


def _fill(
    *,
    message: str,
    tool: str,
    memory_messages: list,
    available_tools: str,
    mlx_complete,
    base: PlanResponse,
) -> PlanResponse:
    fill_messages = list(memory_messages) + [
        {
            "role": "user",
            "content": (
                f"Fill tool_input for tool `{tool}` from the user message. "
                f"Omit unknown required fields.\nUser message:\n{message}"
            ),
        }
    ]
    try:
        raw = mlx_complete(
            build_fill_system_prompt(available_tools, tool),
            fill_messages,
        )
        plan = parse_plan(raw, message)
        plan.intent = "tool_use"
        plan.tool = tool
        # Preserve classify-side preference/decision if fill omitted them.
        if not plan.preference_detected and base.preference_detected:
            plan.preference_detected = base.preference_detected
        if not plan.decision_detected and base.decision_detected:
            plan.decision_detected = base.decision_detected
        if not plan.task_state and base.task_state:
            plan.task_state = base.task_state
        if not plan.mode_hint and base.mode_hint:
            plan.mode_hint = base.mode_hint
        if not plan.reasoning:
            plan.reasoning = base.reasoning
        return normalize_plan(plan)
    except Exception as e:
        logger.warning("fill mlx failed, heuristic fill repair: %s", e)
        return normalize_plan(
            PlanResponse(
                intent="tool_use",
                tool=tool,
                tool_input=heuristic_fill_for_tool(tool, message),
                reasoning=base.reasoning
                or f"Fill repair after MLX error ({type(e).__name__}).",
                response=None,
                task_state=base.task_state,
                mode_hint=base.mode_hint,
                preference_detected=base.preference_detected,
                decision_detected=base.decision_detected,
                next="tool",
            )
        )


def _continue_step(
    *,
    message: str,
    history: list,
    memory_messages: list,
    available_tools: str,
    mlx_complete,
    scratchpad: list[dict[str, Any]],
) -> PlanResponse:
    pad = json.dumps(scratchpad, ensure_ascii=False)[:6000]
    cont_messages = list(memory_messages) + [
        {
            "role": "user",
            "content": (
                f"User goal:\n{message}\n\nScratchpad (prior tool steps):\n{pad}\n\n"
                "Decide next: ask | tool | finish."
            ),
        }
    ]
    try:
        raw = mlx_complete(
            build_continue_system_prompt(available_tools),
            cont_messages,
        )
        plan = parse_plan(raw, message)
        if plan.next == "tool" and plan.tool:
            intent, tool = repair_classified_tool(
                "tool_use",
                plan.tool,
                message,
                _history_for_repair(history),
            )
            plan.intent = intent
            plan.tool = tool
        return normalize_plan(plan)
    except Exception as e:
        logger.warning("continue mlx failed, finishing: %s", e)
        last = scratchpad[-1] if scratchpad else {}
        summary = last.get("summary") or last.get("output") or "Done."
        return normalize_plan(
            PlanResponse(
                intent="chat",
                tool=None,
                tool_input=None,
                reasoning=f"Continue fallback ({type(e).__name__}).",
                response=str(summary)[:2000],
                next="finish",
                task_state="completed",
            )
        )
