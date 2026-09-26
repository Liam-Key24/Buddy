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
    "calendar_delete",
    "calendar_update",
    "calendar_move",
    "session_outcome",
    "spark_capture",
    "spark_promote",
    "spark_dismiss",
]

CalendarOp = Literal["delete", "update", "move", "mark_outcome"]

AnswerType = Literal[
    "short_text",
    "number",
    "date",
    "time",
    "single_choice",
    "multiple_choice",
    "yes_no",
]


class ClarificationQuestion(BaseModel):
    id: str
    label: str
    help_text: str | None = None
    answer_type: AnswerType = "short_text"
    required: bool = True
    options: list[str] = Field(default_factory=list)
    suggested_answer: str | None = None
    reason: str | None = None


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
    created_at: str | None = None
    updated_at: str | None = None


class GoalPublic(Goal):
    """Goal plus session stats for the Goals page and detail panel."""

    events_total: int = 0
    events_completed: int = 0
    events_missed: int = 0
    started_at: str | None = None
    ended_at: str | None = None


class Message(BaseModel):
    id: str
    conversation_id: str
    role: Literal["user", "assistant", "system"]
    content: str
    created_at: str


class CategoryBrief(BaseModel):
    id: str
    name: str
    color: str
    icon: str = "circle"
    keywords: str = ""


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
    category_id: str | None = None
    category: CategoryBrief | None = None
    updated_at: str | None = None


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


class CalendarAction(BaseModel):
    """One calendar mutation requested from Chat."""

    op: CalendarOp
    session_id: str | None = None
    title_contains: str | None = None
    date: str | None = None  # YYYY-MM-DD
    goal_id: str | None = None
    statuses: list[str] = Field(default_factory=list)  # empty = any status
    all_matching: bool = False
    new_title: str | None = None
    new_start_at: str | None = None
    new_end_at: str | None = None
    outcome: Literal["completed", "missed"] | None = None
    notes: str | None = None


class RequestedAction(BaseModel):
    type: str = "none"
    batch_id: str | None = None
    session_id: str | None = None
    outcome: Literal["completed", "missed"] | None = None
    spark_id: str | None = None
    spark_content: str | None = None


KNOWN_REQUESTED_ACTIONS = {
    "propose_sessions",
    "approve_proposals",
    "reject_proposals",
    "none",
}

OPERATION_KINDS = {
    "goal_create",
    "goal_update",
    "pause_others",
    "propose_sessions",
    "approve_proposals",
    "reject_proposals",
    "spark_capture",
    "spark_dismiss",
    "spark_promote",
    "session_outcome",
    "calendar_delete",
    "calendar_update",
    "calendar_move",
    "chat",
}

OperationDisposition = Literal["commit", "clarify", "needs_approval"]


class TurnOperation(BaseModel):
    """One interpreted action in a BuddyTurn. Versioned via BuddyTurn.schema_version."""

    id: str | None = None
    kind: str
    target_type: str | None = None
    target_id: str | None = None
    target_ref: str | None = None
    payload: dict[str, Any] = Field(default_factory=dict)
    assumptions: list[str] = Field(default_factory=list)
    confidence: float = 0.5
    disposition: OperationDisposition | None = None


class BuddyTurn(BaseModel):
    """Canonical structured result from one Cloud AI call."""

    schema_version: int = 1
    assistant_text: str
    intents: list[Intent] = Field(default_factory=lambda: ["chat"])
    operations: list[TurnOperation] = Field(default_factory=list)
    goal_updates: list[GoalUpdate] = Field(default_factory=list)
    clarification: str | None = None
    clarification_questions: list[ClarificationQuestion] = Field(default_factory=list)
    requested_action: RequestedAction | None = None
    calendar_actions: list[CalendarAction] = Field(default_factory=list)
    confidence: float = 0.5
    validation_errors: list[dict[str, Any]] = Field(default_factory=list)


class ProposalGroup(BaseModel):
    goal: Goal | None = None
    proposal_batch_id: str
    summary: dict[str, Any] | None = None
    sessions: list[SessionOut] = Field(default_factory=list)


class ClarificationAnswer(BaseModel):
    question_id: str
    answer: str


class ChatRequest(BaseModel):
    message: str
    conversation_id: str | None = None
    request_id: str | None = None
    clarification_answers: list[ClarificationAnswer] = Field(default_factory=list)
    revision_of: str | None = None


class ChatCancelRequest(BaseModel):
    request_id: str


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
    proposal_summary: dict[str, Any] | None = None
    clarification_questions: list[ClarificationQuestion] = Field(default_factory=list)
    activity: list[dict[str, Any]] = Field(default_factory=list)
    undo_batch_id: str | None = None
    request_id: str | None = None
    deleted_session_ids: list[str] = Field(default_factory=list)
    updated_sessions: list[SessionOut] = Field(default_factory=list)
    operations: list[dict[str, Any]] = Field(default_factory=list)
    mutation_preview: dict[str, Any] | None = None
    proposal_groups: list[ProposalGroup] = Field(default_factory=list)
    stopped: bool = False
    stop_committed: bool = False


class SessionUpdate(BaseModel):
    title: str | None = None
    start_at: str | None = None
    end_at: str | None = None
    category_id: str | None = None


class TodayNeed(BaseModel):
    id: str
    kind: Literal["gathering", "approve"]
    title: str
    detail: str | None = None
    goal_id: str
    conversation_id: str
    proposal_batch_id: str | None = None


class TodayResponse(BaseModel):
    goals: list[Goal]
    attention: list[str]
    pending_questions: list[str]
    needs: list[TodayNeed] = Field(default_factory=list)
    todays_sessions: list[SessionOut] = Field(default_factory=list)
    progress: list[dict[str, Any]] = Field(default_factory=list)
    resurfaced_spark: Spark | None = None


class ProposalDecision(BaseModel):
    batch_id: str
    decision: Literal["approve", "reject", "adjust", "undo"]
    note: str | None = None
    conversation_id: str | None = None


class OutcomeRequest(BaseModel):
    session_id: str
    outcome: Literal["completed", "missed"]
    notes: str | None = None


class SparkCreate(BaseModel):
    content: str


class SparkPromote(BaseModel):
    goal_id: str | None = None
    message: str | None = None
