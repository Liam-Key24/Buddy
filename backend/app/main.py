"""Buddy backend — localhost FastAPI, SQLite, Cloud AI."""

from __future__ import annotations

from typing import Any

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field

from .config import load_settings
from .control_plane import ControlPlane
from .schemas import (
    ChatRequest,
    ChatResponse,
    OutcomeRequest,
    ProposalDecision,
    SparkCreate,
    SparkPromote,
    TodayResponse,
)

settings = load_settings()
app = FastAPI(title="Buddy", version="0.4.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=[
        "http://localhost:5173",
        "http://127.0.0.1:5173",
        "tauri://localhost",
        "https://tauri.localhost",
    ],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

plane = ControlPlane(settings=settings)


@app.on_event("shutdown")
def _shutdown():
    plane.close()


@app.get("/health")
def health():
    return {
        "ok": True,
        "product": "Buddy",
        "ai": {
            "label": "Cloud AI",
            "enabled": settings.ai_enabled,
            "configured": bool(settings.groq_api_key),
            "model_configured": settings.groq_model,
        },
        "db": str(settings.db_path),
        "host": settings.host,
        "privacy": "Local data · Cloud reasoning",
    }


@app.post("/chat", response_model=ChatResponse)
def chat(req: ChatRequest) -> ChatResponse:
    if not req.message.strip():
        raise HTTPException(status_code=400, detail="Message required")
    return plane.handle_message(req.message, req.conversation_id)


@app.get("/today", response_model=TodayResponse)
def today() -> TodayResponse:
    return plane.get_today()


@app.get("/calendar/sessions")
def calendar_sessions(start: str | None = None, end: str | None = None):
    return plane.calendar.list_sessions(start=start, end=end)


@app.get("/calendar/fixed")
def calendar_fixed():
    return plane.calendar.list_fixed_blocks()


@app.post("/calendar/proposals/decide")
def decide_proposal(body: ProposalDecision):
    return plane.decide_proposal(
        body.batch_id, body.decision, conversation_id=body.conversation_id
    )


@app.post("/calendar/sessions/outcome")
def session_outcome(body: OutcomeRequest):
    updated = plane.mark_outcome(body.session_id, body.outcome, body.notes)
    if not updated:
        raise HTTPException(status_code=404, detail="Session not found")
    return updated


@app.get("/sparks")
def list_sparks(open_only: bool = True):
    return plane.sparks.list_open() if open_only else plane.sparks.list_all()


@app.post("/sparks")
def create_spark(body: SparkCreate):
    if not body.content.strip():
        raise HTTPException(status_code=400, detail="Content required")
    return plane.sparks.capture(body.content)


@app.post("/sparks/{spark_id}/promote")
def promote_spark(spark_id: str, body: SparkPromote):
    spark_row = plane.conn.execute("SELECT * FROM sparks WHERE id = ?", (spark_id,)).fetchone()
    if not spark_row:
        raise HTTPException(status_code=404, detail="Spark not found")
    if body.goal_id:
        return plane.sparks.promote(spark_id, body.goal_id)
    seed = body.message or spark_row["content"]
    result = plane.handle_message(f"I want to work on this idea: {seed}")
    if result.goal:
        plane.sparks.promote(spark_id, result.goal.id)
    return {"chat": result, "spark_id": spark_id}


@app.post("/sparks/{spark_id}/dismiss")
def dismiss_spark(spark_id: str):
    spark = plane.sparks.dismiss(spark_id)
    if not spark:
        raise HTTPException(status_code=404, detail="Spark not found")
    return spark


class ConversationRename(BaseModel):
    title: str


class DraftBody(BaseModel):
    draft: dict[str, Any] = Field(default_factory=dict)


@app.get("/conversations")
def list_conversations():
    return plane.list_conversations()


@app.post("/conversations")
def create_conversation():
    return plane.create_conversation()


@app.patch("/conversations/{conversation_id}")
def rename_conversation(conversation_id: str, body: ConversationRename):
    row = plane.rename_conversation(conversation_id, body.title)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@app.delete("/conversations/{conversation_id}")
def delete_conversation(conversation_id: str):
    row = plane.delete_conversation(conversation_id)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@app.post("/conversations/{conversation_id}/restore")
def restore_conversation(conversation_id: str):
    row = plane.restore_conversation(conversation_id)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@app.put("/conversations/{conversation_id}/draft")
def save_draft(conversation_id: str, body: DraftBody):
    row = plane.save_draft(conversation_id, body.draft)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation not found")
    return row


@app.get("/conversations/{conversation_id}/messages")
def messages(conversation_id: str):
    return plane.list_messages(conversation_id)


@app.get("/ai/usage")
def ai_usage():
    return plane.usage_today()


@app.get("/goals/{goal_id}")
def get_goal(goal_id: str):
    goal = plane.get_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal
