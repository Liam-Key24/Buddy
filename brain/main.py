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
from parser import parse_extraction, parse_plan
from prompts import PLAN_SYSTEM_PROMPT, RESPOND_SYSTEM_PROMPT
from prompts.memory import EXTRACTION_PROMPTS

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


class PlanResponse(BaseModel):
    intent: str
    tool: Optional[str] = None
    tool_input: Optional[str] = None
    reasoning: str
    response: Optional[str] = None
    task_state: Optional[str] = None
    preference_detected: Optional[dict] = None
    decision_detected: Optional[dict] = None


class RespondRequest(BaseModel):
    message: str
    history: list[HistoryMessage] = []
    memory: MemoryContextPayload = MemoryContextPayload()
    tool_name: Optional[str] = None
    tool_result: Optional[str] = None


class ExtractRequest(BaseModel):
    kind: Literal["handover", "reflection", "project", "preference", "decision"]
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


@app.get("/health")
def health():
    return {"status": "ok"}


@app.post("/chat/plan", response_model=PlanResponse)
def chat_plan(req: PlanRequest):
    logger.info("plan request: %s", req.message[:80])
    start = time.time()

    messages = build_messages(req.memory, req.history, req.message)

    try:
        raw = mlx.complete(
            system=PLAN_SYSTEM_PROMPT,
            messages=messages,
            max_tokens=512,
            temperature=0.1,
        )
        plan = parse_plan(raw, req.message)
    except Exception as e:
        logger.warning("mlx unavailable, using heuristic plan: %s", e)
        plan = parse_plan("", req.message)

    logger.info(
        "plan parsed intent=%s tool=%s latency_ms=%d",
        plan.intent,
        plan.tool,
        int((time.time() - start) * 1000),
    )
    return PlanResponse(
        intent=plan.intent,
        tool=plan.tool,
        tool_input=plan.tool_input,
        reasoning=plan.reasoning,
        response=plan.response,
        task_state=plan.task_state,
        preference_detected=plan.preference_detected.model_dump()
        if plan.preference_detected
        else None,
        decision_detected=plan.decision_detected.model_dump()
        if plan.decision_detected
        else None,
    )


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
                yield "I'm Buddy, your local assistant. Start the MLX server for full AI responses."

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
