import json
import logging
import os
import re
import time
from pathlib import Path
from typing import Literal, Optional

from fastapi import FastAPI
from fastapi.responses import StreamingResponse
from pydantic import BaseModel, Field, field_validator

from complete import strip_think
from context import MemoryContextPayload, build_messages
from embeddings import embed_text, embedding_dimensions, preload_model
from mlx_client import (
    AGENT_MAX_TOKENS,
    AGENT_TEMPERATURE,
    CHAT_MAX_TOKENS,
    NARRATE_MAX_TOKENS,
    MLXClient,
)
from parser import (
    CALENDAR_SCHEDULE_TASK,
    apply_respond_mode,
    parse_extraction,
    _looks_like_week_plan,
    _matches_any,
    _parse_multi_schedule_tasks,
)
from plan_pipeline import run_classify_fill
from prompts import respond_system_prompt
from prompts.memory import EXTRACTION_PROMPTS

MAX_MESSAGE_CHARS = 32_000
MAX_HISTORY_ITEMS = 64
MAX_COMPLETE_MESSAGES = 80
MAX_EMBED_CHARS = 16_000


def _schedule_source_text(req: "PlanRequest") -> str:
    """Prefer prior user content when the message is 'add this/these to my calendar'."""
    msg = req.message.strip()
    lower = msg.lower()
    if not re.search(
        r"\badd (?:this|that|it|these|them) to (?:my |the )?(?:calend(?:a|e)r)\b",
        lower,
    ):
        return msg

    def _score_prior(content: str) -> int:
        c = content.strip()
        if not c:
            return -1
        score = 0
        if _looks_like_week_plan(c) or _parse_multi_schedule_tasks(c):
            score += 100
        if _matches_any(c, CALENDAR_SCHEDULE_TASK):
            score += 50
        if re.search(
            r"\b(?:session|climbing|study|gym|yoga|schedule|book|plan)\b",
            c,
            flags=re.IGNORECASE,
        ):
            score += 10
        # Prefer richer scheduling requests over short follow-ups.
        score += min(len(c) // 20, 5)
        return score

    best: str | None = None
    best_score = 0
    for item in reversed(req.history or []):
        if getattr(item, "role", "") != "user":
            continue
        content = (item.content or "").strip()
        if not content:
            continue
        # Skip other add-to-calendar echoes.
        if re.search(
            r"\badd (?:this|that|it|these|them) to (?:my |the )?(?:calend(?:a|e)r)\b",
            content.lower(),
        ):
            continue
        s = _score_prior(content)
        if s > best_score:
            best_score = s
            best = content
    if best and best_score >= 10:
        return best
    # Fallback: latest prior user message (legacy behaviour).
    for item in reversed(req.history or []):
        if getattr(item, "role", "") == "user" and item.content.strip():
            return item.content.strip()
    return msg

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
    "BUDDY_MODEL", "mlx-community/Qwen3-14B-4bit"
)

app = FastAPI(title="Buddy Brain")
mlx = MLXClient(base_url=MLX_BASE_URL, model=MODEL)

BRAIN_API = 2
BRAIN_ROUTES = [
    "/chat/plan",
    "/chat/talk",
    "/chat/respond",
    "/v1/complete",
    "/memory/extract",
    "/embed",
]


@app.on_event("startup")
def _preload_embeddings():
    preload_model()


class HistoryMessage(BaseModel):
    role: str
    content: str = Field(..., max_length=MAX_MESSAGE_CHARS)


class PlanRequest(BaseModel):
    message: str = Field(..., max_length=MAX_MESSAGE_CHARS)
    history: list[HistoryMessage] = Field(default_factory=list, max_length=MAX_HISTORY_ITEMS)
    memory: MemoryContextPayload = MemoryContextPayload()
    available_tools: str = ""
    # Agent-loop continue: prior tool steps [{tool, summary, output?}].
    scratchpad: list[dict] = []
    # Probe heuristics only — never call MLX (used before native loop).
    heuristic_only: bool = False


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
    next: Optional[str] = None


class RespondRequest(BaseModel):
    message: str = Field(..., max_length=MAX_MESSAGE_CHARS)
    history: list[HistoryMessage] = Field(default_factory=list, max_length=MAX_HISTORY_ITEMS)
    memory: MemoryContextPayload = MemoryContextPayload()
    tool_name: Optional[str] = None
    tool_result: Optional[str] = Field(default=None, max_length=MAX_MESSAGE_CHARS)


class TalkRequest(BaseModel):
    message: str = Field(..., max_length=MAX_MESSAGE_CHARS)
    history: list[HistoryMessage] = Field(default_factory=list, max_length=MAX_HISTORY_ITEMS)
    model: Optional[str] = None
    max_tokens: int = CHAT_MAX_TOKENS


class CompleteMessage(BaseModel):
    role: str
    content: Optional[str] = Field(default=None, max_length=MAX_MESSAGE_CHARS)
    name: Optional[str] = None
    tool_call_id: Optional[str] = None
    tool_calls: Optional[list[dict]] = None


class CompleteRequest(BaseModel):
    messages: list[dict] = Field(default_factory=list, max_length=MAX_COMPLETE_MESSAGES)
    tools: list[dict] = Field(default_factory=list, max_length=80)
    max_tokens: int = AGENT_MAX_TOKENS
    temperature: float = AGENT_TEMPERATURE

    @field_validator("messages")
    @classmethod
    def _cap_message_content(cls, messages: list[dict]) -> list[dict]:
        for msg in messages:
            content = msg.get("content")
            if isinstance(content, str) and len(content) > MAX_MESSAGE_CHARS:
                msg["content"] = content[:MAX_MESSAGE_CHARS]
        return messages


class CompleteToolCall(BaseModel):
    id: str
    name: str
    arguments: dict = {}


class CompleteResponse(BaseModel):
    content: Optional[str] = None
    tool_calls: list[CompleteToolCall] = []
    finish_reason: str = "stop"


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
    workspace_summary: str = Field(..., max_length=MAX_MESSAGE_CHARS)
    recent_messages: list[HistoryMessage] = Field(default_factory=list, max_length=MAX_HISTORY_ITEMS)
    task_outcome: Optional[str] = Field(default=None, max_length=MAX_MESSAGE_CHARS)


class ExtractResponse(BaseModel):
    kind: str
    data: dict


class EmbedRequest(BaseModel):
    text: str = Field(..., max_length=MAX_EMBED_CHARS)


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
    return {"status": "ok", "api": BRAIN_API, "routes": list(BRAIN_ROUTES)}


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
        next=plan.next,
    )


@app.post("/chat/plan", response_model=PlanResponse)
def chat_plan(req: PlanRequest):
    """Eval / repair only — production turns use Rust route() + `/v1/complete`."""
    logger.info("plan request: %s", req.message[:80])
    start = time.time()

    source = _schedule_source_text(req)
    messages = build_messages(req.memory, req.history, source)

    def mlx_complete(system: str, msgs: list) -> str:
        return mlx.complete(
            system=system,
            messages=msgs,
            max_tokens=AGENT_MAX_TOKENS,
            temperature=0.1,
        )

    plan = run_classify_fill(
        message=source,
        history=req.history or [],
        memory_messages=messages,
        available_tools=req.available_tools,
        mlx_complete=mlx_complete,
        scratchpad=None if req.heuristic_only else (req.scratchpad or None),
        allow_mlx=not req.heuristic_only,
    )

    # Surface MLX connection issues on pure chat fallbacks.
    if (
        plan.intent == "chat"
        and plan.reasoning
        and "MLX error" in plan.reasoning
        and plan.response
    ):
        plan.response = f"{_planner_error_notice(RuntimeError(plan.reasoning))} {plan.response}"

    logger.info(
        "plan parsed intent=%s tool=%s next=%s respond_mode=%s heuristic_only=%s latency_ms=%d",
        plan.intent,
        plan.tool,
        plan.next,
        plan.respond_mode,
        req.heuristic_only,
        int((time.time() - start) * 1000),
    )
    return _plan_to_http(plan)


TALK_SYSTEM = (
    "You are Buddy, a helpful local assistant. Answer briefly and clearly in plain text. "
    "No tools, no JSON, no tags."
)


@app.post("/chat/talk")
def chat_talk(req: TalkRequest):
    """Fast Llama chat — tiny prompt, streamed tokens, no tool loop."""
    logger.info("talk request chars=%d", len(req.message or ""))
    messages = [
        {"role": item.role, "content": item.content}
        for item in (req.history or [])[-8:]
        if item.role in ("user", "assistant") and (item.content or "").strip()
    ]
    messages.append({"role": "user", "content": req.message})

    def generate():
        start = time.time()
        chars = 0
        try:
            for chunk in mlx.stream_simple(
                system=TALK_SYSTEM,
                messages=messages,
                max_tokens=min(req.max_tokens, CHAT_MAX_TOKENS),
                temperature=0.7,
                model=req.model,
            ):
                chars += len(chunk)
                yield chunk
        except Exception as e:
            logger.warning("talk stream failed: %s", e)
            yield (
                "I couldn't reach the local chat model. "
                "Try again in a moment."
            )
        finally:
            logger.info(
                "talk finish chars=%d latency_ms=%d",
                chars,
                int((time.time() - start) * 1000),
            )

    return StreamingResponse(
        generate(),
        media_type="text/plain",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


@app.post("/chat/respond")
def chat_respond(req: RespondRequest):
    logger.info("respond request tool=%s", req.tool_name)

    has_tool = bool(req.tool_name and req.tool_result is not None)
    messages = build_messages(
        req.memory,
        req.history,
        req.message,
        include_memory=has_tool,
        history_limit=8 if not has_tool else 16,
    )
    if has_tool:
        messages.append(
            {
                "role": "assistant",
                "content": f"[Tool {req.tool_name} returned: {req.tool_result}]",
            }
        )
    max_tokens = NARRATE_MAX_TOKENS if has_tool else CHAT_MAX_TOKENS

    def generate():
        start = time.time()
        chars = 0
        try:
            for chunk in mlx.stream(
                system=respond_system_prompt(
                    has_tool_result=has_tool,
                    tool_name=req.tool_name if has_tool else None,
                ),
                messages=messages,
                max_tokens=max_tokens,
                temperature=0.7 if not has_tool else 0.4,
            ):
                chars += len(chunk)
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
        finally:
            logger.info(
                "respond finish tool=%s chars=%d max_tokens=%d latency_ms=%d",
                req.tool_name,
                chars,
                max_tokens,
                int((time.time() - start) * 1000),
            )

    return StreamingResponse(
        generate(),
        media_type="text/plain",
        headers={"Cache-Control": "no-cache", "X-Accel-Buffering": "no"},
    )


@app.post("/v1/complete", response_model=CompleteResponse)
def v1_complete(req: CompleteRequest):
    logger.info(
        "complete request messages=%d tools=%d",
        len(req.messages or []),
        len(req.tools or []),
    )
    start = time.time()
    try:
        result = mlx.complete_with_tools(
            messages=req.messages,
            tools=req.tools or None,
            max_tokens=req.max_tokens or AGENT_MAX_TOKENS,
            temperature=req.temperature if req.temperature is not None else AGENT_TEMPERATURE,
        )
    except Exception as e:
        logger.warning("complete failed: %s", e)
        raise
    content = strip_think(result.get("content"))
    tool_calls = [
        CompleteToolCall(
            id=str(tc.get("id") or f"call_{i}"),
            name=str(tc.get("name") or ""),
            arguments=tc.get("arguments") if isinstance(tc.get("arguments"), dict) else {},
        )
        for i, tc in enumerate(result.get("tool_calls") or [])
        if tc.get("name")
    ]
    logger.info(
        "complete finish=%s tool_calls=%d latency_ms=%d",
        result.get("finish_reason"),
        len(tool_calls),
        int((time.time() - start) * 1000),
    )
    return CompleteResponse(
        content=content or None,
        tool_calls=tool_calls,
        finish_reason=str(result.get("finish_reason") or ("tool_calls" if tool_calls else "stop")),
    )


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
