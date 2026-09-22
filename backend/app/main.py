"""Buddy backend — localhost FastAPI, SQLite, Cloud AI."""

from __future__ import annotations

from typing import Any

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field

from .config import load_settings
from .control_plane import ControlPlane, RevertBlocked
from .schemas import (
    ChatCancelRequest,
    ChatRequest,
    ChatResponse,
    OutcomeRequest,
    ProposalDecision,
    SessionUpdate,
    SparkCreate,
    SparkPromote,
    TodayResponse,
)

settings = load_settings()
app = FastAPI(title="Buddy", version="0.4.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=list(settings.cors_origins),
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
    try:
        return plane.handle_message(
            req.message,
            req.conversation_id,
            request_id=req.request_id,
            clarification_answers=req.clarification_answers,
            revision_of=req.revision_of,
        )
    except RevertBlocked as exc:
        raise HTTPException(status_code=409, detail=exc.detail) from exc


@app.post("/chat/cancel")
def chat_cancel(body: ChatCancelRequest):
    if not body.request_id.strip():
        raise HTTPException(status_code=400, detail="request_id required")
    return plane.cancel_request(body.request_id.strip())


@app.get("/today", response_model=TodayResponse)
def today() -> TodayResponse:
    return plane.get_today()


@app.get("/calendar/sessions")
def calendar_sessions(start: str | None = None, end: str | None = None):
    return plane.calendar.list_sessions(start=start, end=end)


@app.get("/calendar/fixed")
def calendar_fixed():
    return plane.calendar.list_fixed_blocks()


class FixedBlockCreate(BaseModel):
    title: str
    weekday: int
    start_minute: int
    end_minute: int


class FixedBlockUpdate(BaseModel):
    title: str | None = None
    weekday: int | None = None
    start_minute: int | None = None
    end_minute: int | None = None


@app.post("/calendar/fixed")
def create_fixed(body: FixedBlockCreate):
    try:
        return plane.calendar.create_fixed_block(
            title=body.title,
            weekday=body.weekday,
            start_minute=body.start_minute,
            end_minute=body.end_minute,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@app.patch("/calendar/fixed/{block_id}")
def update_fixed(block_id: str, body: FixedBlockUpdate):
    try:
        updated = plane.calendar.update_fixed_block(
            block_id,
            title=body.title,
            weekday=body.weekday,
            start_minute=body.start_minute,
            end_minute=body.end_minute,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    if not updated:
        raise HTTPException(status_code=404, detail="Fixed block not found")
    return updated


@app.delete("/calendar/fixed/{block_id}")
def delete_fixed(block_id: str):
    if not plane.calendar.delete_fixed_block(block_id):
        raise HTTPException(status_code=404, detail="Fixed block not found")
    return {"ok": True}


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


class SessionCreate(BaseModel):
    title: str
    start_at: str
    end_at: str
    category_id: str | None = None


@app.post("/calendar/sessions")
def create_session(body: SessionCreate):
    try:
        return plane.calendar.create_manual_session(
            title=body.title,
            start_at=body.start_at,
            end_at=body.end_at,
            category_id=body.category_id,
        )
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@app.delete("/calendar/sessions/{session_id}")
def delete_session(session_id: str):
    if not plane.calendar.delete_session(session_id):
        raise HTTPException(status_code=404, detail="Session not found")
    return {"ok": True}


@app.patch("/calendar/sessions/{session_id}")
def update_session(session_id: str, body: SessionUpdate):
    try:
        data = body.model_dump(exclude_unset=True)
        updated = plane.calendar.update_session(session_id, **data)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    if not updated:
        raise HTTPException(status_code=404, detail="Session not found")
    return updated


class CategoryCreate(BaseModel):
    name: str
    color: str = "#93c5fd"
    icon: str = "circle"
    keywords: str = ""


class CategoryUpdate(BaseModel):
    name: str | None = None
    color: str | None = None
    icon: str | None = None
    keywords: str | None = None


@app.get("/categories")
def list_categories():
    return plane.categories.list()


@app.post("/categories")
def create_category(body: CategoryCreate):
    if not body.name.strip():
        raise HTTPException(status_code=400, detail="Name required")
    cat = plane.categories.create(
        name=body.name,
        color=body.color,
        icon=body.icon,
        keywords=body.keywords,
    )
    plane.calendar.reclassify_all()
    return cat


@app.patch("/categories/{category_id}")
def update_category(category_id: str, body: CategoryUpdate):
    fields = {k: v for k, v in body.model_dump().items() if v is not None}
    cat = plane.categories.update(category_id, **fields)
    if not cat:
        raise HTTPException(status_code=404, detail="Category not found")
    plane.calendar.reclassify_all()
    return cat


@app.delete("/categories/{category_id}")
def delete_category(category_id: str):
    if not plane.categories.delete(category_id):
        raise HTTPException(status_code=404, detail="Category not found")
    plane.calendar.reclassify_all()
    return {"ok": True}


@app.post("/categories/reclassify")
def reclassify_sessions():
    return {"updated": plane.calendar.reclassify_all()}


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


class ConversationPatch(BaseModel):
    title: str | None = None
    folder_id: str | None = None


class ConversationPlace(BaseModel):
    folder_id: str | None = None
    before_id: str | None = None


class FolderCreate(BaseModel):
    title: str


class FolderRename(BaseModel):
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
def patch_conversation(conversation_id: str, body: ConversationPatch):
    row = None
    if body.title is not None:
        row = plane.rename_conversation(conversation_id, body.title)
        if not row:
            raise HTTPException(status_code=404, detail="Conversation not found")
    if "folder_id" in body.model_fields_set:
        row = plane.move_conversation(conversation_id, body.folder_id)
        if not row:
            raise HTTPException(status_code=404, detail="Conversation or folder not found")
    if row is None:
        raise HTTPException(status_code=400, detail="Nothing to update")
    return row


@app.post("/conversations/{conversation_id}/place")
def place_conversation(conversation_id: str, body: ConversationPlace):
    row = plane.place_conversation(conversation_id, body.folder_id, body.before_id)
    if not row:
        raise HTTPException(status_code=404, detail="Conversation or folder not found")
    return row


@app.get("/folders")
def list_folders():
    return plane.list_folders()


@app.post("/folders")
def create_folder(body: FolderCreate):
    if not body.title.strip():
        raise HTTPException(status_code=400, detail="Title required")
    return plane.create_folder(body.title)


@app.patch("/folders/{folder_id}")
def rename_folder(folder_id: str, body: FolderRename):
    row = plane.rename_folder(folder_id, body.title)
    if not row:
        raise HTTPException(status_code=404, detail="Folder not found")
    return row


@app.delete("/folders/{folder_id}")
def delete_folder(folder_id: str):
    row = plane.delete_folder(folder_id)
    if not row:
        raise HTTPException(status_code=404, detail="Folder not found")
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


@app.post("/conversations/{conversation_id}/messages/{message_id}/revert")
def revert_message(conversation_id: str, message_id: str):
    result = plane.revert_to(conversation_id, message_id)
    if result.get("blocked"):
        raise HTTPException(status_code=409, detail=result.get("detail") or "Can't revert that message.")
    if not result.get("ok"):
        raise HTTPException(status_code=404, detail="Message not found")
    return result


@app.get("/conversations/{conversation_id}/messages")
def messages(conversation_id: str):
    return plane.list_messages(conversation_id)


@app.get("/conversations/{conversation_id}/open-proposal")
def open_proposal(conversation_id: str):
    return plane.get_open_proposal(conversation_id)


@app.get("/ai/usage")
def ai_usage():
    return plane.usage_today()


@app.get("/goals")
def list_goals():
    return plane.list_goals()


@app.get("/goals/{goal_id}")
def get_goal(goal_id: str):
    goal = plane.get_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal


@app.delete("/goals/{goal_id}")
def delete_goal(goal_id: str):
    goal = plane.remove_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal
