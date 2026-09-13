from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, Field


GoalStatus = Literal["gathering", "ready_to_plan", "planned", "active", "paused", "done"]


class Goal(BaseModel):
    id: str
    conversation_id: str
    title: str
    domain: str | None = None
    target: str | None = None
    deadline: str | None = None
    baseline: str | None = None
    frequency: str | None = None
    commitment: str | None = None
    status: GoalStatus = "gathering"
    facts: dict[str, Any] = Field(default_factory=dict)


class Message(BaseModel):
    id: str
    conversation_id: str
    role: Literal["user", "assistant", "system"]
    content: str
    created_at: str


class ChatRequest(BaseModel):
    message: str
    conversation_id: str | None = None


class ChatResponse(BaseModel):
    conversation_id: str
    reply: str
    goal: Goal | None = None
    pending_question: str | None = None


class TodayResponse(BaseModel):
    goals: list[Goal]
    attention: list[str]
    pending_questions: list[str]
