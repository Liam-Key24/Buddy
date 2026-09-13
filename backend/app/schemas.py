from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel, Field


GoalStatus = Literal["gathering", "ready_to_plan", "planned", "active", "paused", "done"]
SessionStatus = Literal["proposed", "scheduled", "completed", "missed", "rejected"]
SessionKind = Literal["fixed", "flexible", "proposed"]
SparkStatus = Literal["open", "promoted", "dismissed", "snoozed"]

Intent = Literal[
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
]


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


class SessionOut(BaseModel):
    id: str
    goal_id: str | None = None
    title: str
    start_at: str
    end_at: str
    kind: str
    status: str
    proposal_batch_id: str | None = None
    notes: str | None = None


class Spark(BaseModel):
    id: str
    content: str
    status: SparkStatus
    promoted_goal_id: str | None = None
    created_at: str


class GoalUpdate(BaseModel):
    action: Literal["create", "update", "pause_others"] = "update"
    title: str | None = None
    domain: str | None = None
    target: str | None = None
    deadline: str | None = None
    baseline: str | None = None
    frequency: str | None = None
    commitment: str | None = None
    status: GoalStatus | None = None
    facts: dict[str, Any] = Field(default_factory=dict)


class RequestedAction(BaseModel):
    type: Literal[
        "propose_sessions",
        "approve_proposals",
        "reject_proposals",
        "none",
    ] = "none"
    batch_id: str | None = None
    session_id: str | None = None
    outcome: Literal["completed", "missed"] | None = None
    spark_id: str | None = None
    spark_content: str | None = None


class BuddyTurn(BaseModel):
    """Canonical structured result from one Cloud AI call."""

    assistant_text: str
    intents: list[Intent] = Field(default_factory=lambda: ["chat"])
    goal_updates: list[GoalUpdate] = Field(default_factory=list)
    clarification: str | None = None
    requested_action: RequestedAction | None = None
    confidence: float = 0.5


class ChatRequest(BaseModel):
    message: str
    conversation_id: str | None = None


class ChatResponse(BaseModel):
    conversation_id: str
    reply: str
    goal: Goal | None = None
    pending_question: str | None = None
    proposed_sessions: list[SessionOut] = Field(default_factory=list)
    booked_sessions: list[SessionOut] = Field(default_factory=list)
    sparks: list[Spark] = Field(default_factory=list)
    unresolved: list[str] = Field(default_factory=list)
    ai_available: bool = True


class TodayResponse(BaseModel):
    goals: list[Goal]
    attention: list[str]
    pending_questions: list[str]
    todays_sessions: list[SessionOut] = Field(default_factory=list)
    progress: list[dict[str, Any]] = Field(default_factory=list)
    resurfaced_spark: Spark | None = None


class ProposalDecision(BaseModel):
    batch_id: str
    decision: Literal["approve", "reject", "adjust"]
    note: str | None = None


class OutcomeRequest(BaseModel):
    session_id: str
    outcome: Literal["completed", "missed"]
    notes: str | None = None


class SparkCreate(BaseModel):
    content: str


class SparkPromote(BaseModel):
    goal_id: str | None = None
    message: str | None = None
