"""Buddy backend — one FastAPI service, one SQLite database, one model."""

from __future__ import annotations

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware

from .goal_conversation import GoalConversationService
from .schemas import (
    ChatRequest,
    ChatResponse,
    OutcomeRequest,
    ProposalDecision,
    SparkCreate,
    SparkPromote,
    TodayResponse,
)

app = FastAPI(title="Buddy", version="0.2.0")
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:5173", "http://127.0.0.1:5173"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

service = GoalConversationService()


@app.get("/health")
def health():
    from .mlx_model import model_status

    return {"ok": True, "product": "Buddy", "model": model_status()}


@app.post("/chat", response_model=ChatResponse)
def chat(req: ChatRequest) -> ChatResponse:
    if not req.message.strip():
        raise HTTPException(status_code=400, detail="Message required")
    return service.handle_message(req.message, req.conversation_id)


@app.get("/today", response_model=TodayResponse)
def today() -> TodayResponse:
    return service.get_today()


@app.get("/calendar/sessions")
def calendar_sessions(start: str | None = None, end: str | None = None):
    return service.calendar.list_sessions(start=start, end=end)


@app.get("/calendar/fixed")
def calendar_fixed():
    return service.calendar.list_fixed_blocks()


@app.post("/calendar/proposals/decide")
def decide_proposal(body: ProposalDecision):
    if body.decision == "approve":
        booked = service.calendar.approve_batch(body.batch_id)
        return {"booked": booked}
    n = service.calendar.reject_batch(body.batch_id)
    return {"rejected": n}


@app.post("/calendar/sessions/outcome")
def session_outcome(body: OutcomeRequest):
    updated = service.calendar.mark_outcome(body.session_id, body.outcome, body.notes)
    if not updated:
        raise HTTPException(status_code=404, detail="Session not found")
    return updated


@app.get("/sparks")
def list_sparks(open_only: bool = True):
    return service.sparks.list_open() if open_only else service.sparks.list_all()


@app.post("/sparks")
def create_spark(body: SparkCreate):
    if not body.content.strip():
        raise HTTPException(status_code=400, detail="Content required")
    return service.sparks.capture(body.content)


@app.post("/sparks/{spark_id}/promote")
def promote_spark(spark_id: str, body: SparkPromote):
    spark_row = service.conn.execute("SELECT * FROM sparks WHERE id = ?", (spark_id,)).fetchone()
    if not spark_row:
        raise HTTPException(status_code=404, detail="Spark not found")
    if body.goal_id:
        promoted = service.sparks.promote(spark_id, body.goal_id)
        return promoted
    # Promote by starting a goal conversation from the spark content.
    seed = body.message or spark_row["content"]
    result = service.handle_message(f"I want to work on this idea: {seed}")
    if result.goal:
        service.sparks.promote(spark_id, result.goal.id)
    return {"chat": result, "spark_id": spark_id}


@app.post("/sparks/{spark_id}/dismiss")
def dismiss_spark(spark_id: str):
    spark = service.sparks.dismiss(spark_id)
    if not spark:
        raise HTTPException(status_code=404, detail="Spark not found")
    return spark


@app.get("/conversations/{conversation_id}/messages")
def messages(conversation_id: str):
    return service.list_messages(conversation_id)


@app.get("/goals/{goal_id}")
def get_goal(goal_id: str):
    goal = service.get_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal
