import json
import logging
import os
import time
from pathlib import Path
from typing import Literal, Optional

from fastapi import FastAPI
from fastapi.responses import StreamingResponse
from pydantic import BaseModel

from context import MemoryContextPayload, build_messages, format_history
from embeddings import embed_text, embedding_dimensions
from mlx_client import MLXClient
from parser import (
    CALENDAR_CREATE,
    CALENDAR_DELETE,
    DREAM_LOG,
    DREAM_SEARCH,
    WORK_SALES,
    WORK_SET_HOURS,
    WORK_STATS,
    _heuristic_plan,
    _matches_any,
    apply_respond_mode,
    normalize_plan,
    parse_extraction,
    parse_plan,
    try_fast_heuristic_plan,
)
from prompts import RESPOND_SYSTEM_PROMPT, build_plan_system_prompt
from prompts.memory import EXTRACTION_PROMPTS


def _schedule_source_text(req: "PlanRequest") -> str:
    """Prefer prior user content when the message is 'add this to my calendar'."""
    msg = req.message.strip()
    lower = msg.lower()
    if re_search_add_this(lower):
        for item in reversed(req.history or []):
            if getattr(item, "role", "") == "user" and item.content.strip():
                return item.content.strip()
    return msg


def re_search_add_this(lower: str) -> bool:
    import re

    return bool(
        re.search(
            r"\badd (?:this|that|it) to (?:my )?(?:calend(?:a|e)r)\b",
            lower,
        )
    )


def _maybe_force_calendar_plan(plan, req: "PlanRequest"):
    """If planner ignored an obvious calendar/lifestyle request, force tools.

    For schedule phrases, prefer the deterministic parser over the model so
    titles/days/times stay accurate.
    """
    source = _schedule_source_text(req)
    schedule_like = _matches_any(source, CALENDAR_CREATE) or _matches_any(
        req.message, CALENDAR_CREATE
    )
    if schedule_like:
        return normalize_plan(_heuristic_plan(source))
    delete_like = _matches_any(source, CALENDAR_DELETE) or _matches_any(
        req.message, CALENDAR_DELETE
    )
    if delete_like:
        return normalize_plan(
            _heuristic_plan(
                source if _matches_any(source, CALENDAR_DELETE) else req.message
            )
        )
    lifestyle_patterns = (
        DREAM_LOG
        + DREAM_SEARCH
        + WORK_SALES
        + WORK_SET_HOURS
        + WORK_STATS
    )
    if _matches_any(req.message, lifestyle_patterns):
        return normalize_plan(_heuristic_plan(req.message))
    return normalize_plan(plan)

LOG_DIR = Path.home() / "Library" / "Logs" / "Buddy"
_handlers: list[logging.Handler] = [logging.StreamHandler()]
try:
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    _handlers.append(logging.FileHandler(LOG_DIR / "brain.log"))
except OSError:
    pass

logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s %(levelname)s %(name)s %(message)s",
    handlers=_handlers,
)
logger = logging.getLogger("buddy.brain")

MLX_BASE_URL = os.environ.get("MLX_BASE_URL", "http://127.0.0.1:8001/v1")
MODEL = os.environ.get(
    "BUDDY_MODEL", "mlx-community/Llama-3.2-3B-Instruct-4bit"
)

app = FastAPI(title="Buddy Brain")
mlx = MLXClient(base_url=MLX_BASE_URL, model=MODEL)


class HistoryMessage(BaseModel):
    role: str
    content: str


class PlanRequest(BaseModel):
    message: str
    history: list[HistoryMessage] = []
    memory: MemoryContextPayload = MemoryContextPayload()
    available_tools: str = ""


class PlanResponse(BaseModel):
    intent: str
    tool: Optional[str] = None
    tool_input: Optional[str] = None
    reasoning: str
    response: Optional[str] = None
    task_state: Optional[str] = None
    mode_hint: Optional[str] = None
    respond_mode: Optional[str] = None
    preference_detected: Optional[dict] = None
    decision_detected: Optional[dict] = None


class RespondRequest(BaseModel):
    message: str
    history: list[HistoryMessage] = []
    memory: MemoryContextPayload = MemoryContextPayload()
    tool_name: Optional[str] = None
    tool_result: Optional[str] = None


class ExtractRequest(BaseModel):
    kind: Literal[
        "handover",
        "reflection",
        "project",
        "preference",
        "decision",
        "conversation_archive",
        "spark_archive",
    ]
    workspace_summary: str
    recent_messages: list[HistoryMessage] = []
    task_outcome: Optional[str] = None


class ExtractResponse(BaseModel):
    kind: str
    data: dict


class EmbedRequest(BaseModel):
    text: str


class EmbedResponse(BaseModel):
    embedding: list[float]
    dimensions: int


def _planner_error_notice(err: Exception) -> str:
    err_type = type(err).__name__
    if err_type == "APIConnectionError":
        return (
            "Heads up: I couldn't reach the MLX model service just now "
            "(connection error)."
        )
    return f"Heads up: the planner hit an error ({err_type})."


def _responder_error_notice(err: Exception) -> str:
    err_type = type(err).__name__
    if err_type == "APIConnectionError":
        return (
            "I couldn't reach the MLX responder right now "
            "(connection error), so this is a fallback reply."
        )
    return f"The responder hit an error ({err_type}), so this is a fallback reply."


@app.get("/health")
def health():
    return {"status": "ok"}


def _plan_to_http(plan) -> PlanResponse:
    plan = apply_respond_mode(plan)
    return PlanResponse(
        intent=plan.intent,
        tool=plan.tool,
        tool_input=plan.tool_input,
        reasoning=plan.reasoning,
        response=plan.response,
        task_state=plan.task_state,
        mode_hint=plan.mode_hint,
        respond_mode=plan.respond_mode,
        preference_detected=plan.preference_detected.model_dump()
        if plan.preference_detected
        else None,
        decision_detected=plan.decision_detected.model_dump()
        if plan.decision_detected
        else None,
    )


def _pending_clarification_active(memory: MemoryContextPayload) -> bool:
    working = (memory.working or "").strip()
    return "Pending clarification" in working


@app.post("/chat/plan", response_model=PlanResponse)
def chat_plan(req: PlanRequest):
    logger.info("plan request: %s", req.message[:80])
    start = time.time()

    # Fast path: confident heuristics skip MLX (Brain-owned; Buddy unchanged).
    if not _pending_clarification_active(req.memory):
        source = _schedule_source_text(req)
        fast = try_fast_heuristic_plan(source)
        if fast is None and source != req.message.strip():
            fast = try_fast_heuristic_plan(req.message)
        if fast is not None:
            fast = _maybe_force_calendar_plan(fast, req)
            logger.info(
                "plan heuristic-fast intent=%s tool=%s respond_mode=%s latency_ms=%d",
                fast.intent,
                fast.tool,
                fast.respond_mode,
                int((time.time() - start) * 1000),
            )
            return _plan_to_http(fast)

    messages = build_messages(req.memory, req.history, req.message)

    raw = ""
    try:
        raw = mlx.complete(
            system=build_plan_system_prompt(req.available_tools),
            messages=messages,
            max_tokens=512,
            temperature=0.1,
        )
        plan = parse_plan(raw, req.message)
        plan = _maybe_force_calendar_plan(plan, req)
    except Exception as e:
        logger.warning("mlx unavailable, using heuristic plan: %s", e)
        plan = parse_plan("", req.message)
        plan = _maybe_force_calendar_plan(plan, req)
        if plan.intent == "chat":
            base_response = plan.response or "I'm here to help."
            plan.response = (
                f"{_planner_error_notice(e)} {base_response} "
                "Please check that MLX is running and reachable."
            )

    plan = apply_respond_mode(plan)
    logger.info(
        "plan parsed intent=%s tool=%s respond_mode=%s latency_ms=%d",
        plan.intent,
        plan.tool,
        plan.respond_mode,
        int((time.time() - start) * 1000),
    )
    return _plan_to_http(plan)


@app.post("/chat/respond")
def chat_respond(req: RespondRequest):
    logger.info("respond request tool=%s", req.tool_name)

    messages = build_messages(req.memory, req.history, req.message)

    if req.tool_name and req.tool_result is not None:
        messages.append(
            {
                "role": "assistant",
                "content": f"[Tool {req.tool_name} returned: {req.tool_result}]",
            }
        )

    def generate():
        try:
            for chunk in mlx.stream(
                system=RESPOND_SYSTEM_PROMPT,
                messages=messages,
                max_tokens=512,
                temperature=0.7,
            ):
                yield chunk
        except Exception as e:
            logger.warning("mlx stream failed, using fallback: %s", e)
            if req.tool_name and req.tool_result is not None:
                yield f"The echo tool returned: {req.tool_result}"
            else:
                yield (
                    f"{_responder_error_notice(e)} "
                    "Start or restart the MLX service for full AI responses."
                )

    return StreamingResponse(generate(), media_type="text/plain")


@app.post("/memory/extract", response_model=ExtractResponse)
def memory_extract(req: ExtractRequest):
    logger.info("memory extract kind=%s", req.kind)
    start = time.time()

    prompt = EXTRACTION_PROMPTS.get(req.kind, EXTRACTION_PROMPTS["handover"])
    context_parts = [f"Workspace context:\n{req.workspace_summary}"]
    if req.task_outcome:
        context_parts.append(f"Task outcome:\n{req.task_outcome}")
    if req.recent_messages:
        history_text = "\n".join(
            f"{m.role}: {m.content}" for m in req.recent_messages[-10:]
        )
        context_parts.append(f"Recent conversation:\n{history_text}")

    user_content = "\n\n".join(context_parts)
    messages = [{"role": "user", "content": user_content}]

    try:
        raw = mlx.complete(
            system=prompt,
            messages=messages,
            max_tokens=1024,
            temperature=0.2,
        )
        data = parse_extraction(req.kind, raw)
    except Exception as e:
        logger.warning("memory extract fallback: %s", e)
        data = parse_extraction(req.kind, "")

    if req.kind == "handover" and "summary" not in data:
        data = {"summary": json.dumps(data)}

    logger.info(
        "memory extract kind=%s latency_ms=%d",
        req.kind,
        int((time.time() - start) * 1000),
    )
    return ExtractResponse(kind=req.kind, data=data)


@app.post("/embed", response_model=EmbedResponse)
def embed(req: EmbedRequest):
    logger.info("embed request len=%d", len(req.text))
    vector = embed_text(req.text)
    return EmbedResponse(embedding=vector, dimensions=embedding_dimensions())


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(app, host="127.0.0.1", port=8002)
