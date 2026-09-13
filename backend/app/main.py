"""Buddy backend — one FastAPI service, one SQLite database."""

from __future__ import annotations

from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware

from .goal_conversation import GoalConversationService
from .schemas import ChatRequest, ChatResponse, TodayResponse

app = FastAPI(title="Buddy", version="0.1.0")
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
    return {"ok": True, "product": "Buddy"}


@app.post("/chat", response_model=ChatResponse)
def chat(req: ChatRequest) -> ChatResponse:
    if not req.message.strip():
        raise HTTPException(status_code=400, detail="Message required")
    return service.handle_message(req.message, req.conversation_id)


@app.get("/today", response_model=TodayResponse)
def today() -> TodayResponse:
    return service.get_today()


@app.get("/conversations/{conversation_id}/messages")
def messages(conversation_id: str):
    return service.list_messages(conversation_id)


@app.get("/goals/{goal_id}")
def get_goal(goal_id: str):
    goal = service.get_goal(goal_id)
    if not goal:
        raise HTTPException(status_code=404, detail="Goal not found")
    return goal
