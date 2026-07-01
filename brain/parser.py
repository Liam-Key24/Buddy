import json
import re
from typing import Optional

from pydantic import BaseModel


class PreferenceDetected(BaseModel):
    key: str
    value: str
    confidence: float
    source: str


class DecisionDetected(BaseModel):
    decision: str
    reason: str


class PlanResponse(BaseModel):
    intent: str
    tool: Optional[str] = None
    tool_input: Optional[str] = None
    reasoning: str
    response: Optional[str] = None
    task_state: Optional[str] = None
    preference_detected: Optional[PreferenceDetected] = None
    decision_detected: Optional[DecisionDetected] = None


def _extract_json(text: str) -> dict:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\n?", "", text)
        text = re.sub(r"\n?```$", "", text)

    start = text.find("{")
    end = text.rfind("}")
    if start >= 0 and end > start:
        text = text[start : end + 1]

    return json.loads(text)


def _heuristic_plan(message: str) -> PlanResponse:
    lower = message.strip().lower()
    if lower.startswith("echo "):
        tool_input = message.strip()[5:].strip()
        return PlanResponse(
            intent="tool_use",
            tool="echo",
            tool_input=tool_input or message.strip(),
            reasoning="User requested echo tool via prefix.",
            response=None,
        )
    if lower == "echo":
        return PlanResponse(
            intent="tool_use",
            tool="echo",
            tool_input="",
            reasoning="User requested echo with no input.",
            response=None,
        )
    return PlanResponse(
        intent="chat",
        tool=None,
        tool_input=None,
        reasoning="General conversation fallback.",
        response="I'm Buddy, your local assistant. How can I help?",
    )


def parse_plan(raw: str, message: str) -> PlanResponse:
    try:
        data = _extract_json(raw)
        pref = data.get("preference_detected")
        dec = data.get("decision_detected")
        return PlanResponse(
            intent=data.get("intent", "chat"),
            tool=data.get("tool"),
            tool_input=data.get("tool_input"),
            reasoning=data.get("reasoning", ""),
            response=data.get("response"),
            task_state=data.get("task_state"),
            preference_detected=PreferenceDetected(**pref) if pref else None,
            decision_detected=DecisionDetected(**dec) if dec else None,
        )
    except (json.JSONDecodeError, KeyError, TypeError):
        return _heuristic_plan(message)


def parse_extraction(kind: str, raw: str) -> dict:
    try:
        return _extract_json(raw)
    except (json.JSONDecodeError, TypeError):
        if kind == "handover":
            return {"summary": raw.strip() or "No handover content generated."}
        if kind == "reflection":
            return {
                "attempted": "Unknown task",
                "successful": False,
                "improvements": "",
                "lessons": raw.strip(),
            }
        if kind == "project":
            return {"section": "general", "content": raw.strip()}
        if kind == "conversation_archive":
            return {
                "summary": raw.strip() or "Archived conversation.",
                "topics": [],
                "key_facts": [],
                "decisions": [],
            }
        return {"raw": raw.strip()}
