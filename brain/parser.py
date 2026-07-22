import json
import re
import time
from typing import Optional

from pydantic import BaseModel


# #region agent log
def _debug_log(location: str, message: str, data: dict, hypothesis_id: str) -> None:
    try:
        with open(
            "/Users/liamgk/Desktop/BUDDY/.cursor/debug-500eec.log", "a"
        ) as f:
            f.write(
                json.dumps(
                    {
                        "sessionId": "500eec",
                        "location": location,
                        "message": message,
                        "data": data,
                        "timestamp": int(time.time() * 1000),
                        "hypothesisId": hypothesis_id,
                    }
                )
                + "\n"
            )
    except OSError:
        pass


def _matched_idea_trigger(message: str) -> str | None:
    lower = message.strip().lower()
    for pattern in IDEA_TRIGGERS:
        if re.search(pattern, lower):
            return pattern
    return None


# #endregion


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


IDEA_TRIGGERS = [
    r"\bi have an idea\b",
    r"\bidea for\b",
    r"\bwhat if we\b",
    r"\bwhat if i\b",
    r"\bthought about\b",
    r"\bthinking about\b",
    r"\bnote to self\b",
    r"\bremember to\b",
    r"\bdon'?t forget\b",
    r"\bmaybe we could\b",
    r"\bmaybe i could\b",
    r"\bwould be (?:cool|nice|good) if\b",
    r"\bi('ve| have) been thinking\b",
    r"\bsomething to (?:try|do|look into)\b",
    r"\bspark:\s*",
    r"\bspark\s+\w",
]


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


def _looks_like_idea(message: str) -> bool:
    lower = message.strip().lower()
    return any(re.search(pattern, lower) for pattern in IDEA_TRIGGERS)


def _extract_idea_content(message: str) -> str:
    text = message.strip()
    lead_ins = [
        r"^i have an idea(?: for)?\s*[:—–-]?\s*",
        r"^idea for\s*[:—–-]?\s*",
        r"^what if we\s*[:—–-]?\s*",
        r"^what if i\s*[:—–-]?\s*",
        r"^note to self\s*[:—–-]?\s*",
        r"^thinking about\s*[:—–-]?\s*",
        r"^thought about\s*[:—–-]?\s*",
        r"^maybe we could\s*[:—–-]?\s*",
        r"^maybe i could\s*[:—–-]?\s*",
        r"^i('ve| have) been thinking(?: about)?\s*[:—–-]?\s*",
        r"^spark:\s*",
        r"^spark\s+",
    ]
    for pattern in lead_ins:
        stripped = re.sub(pattern, "", text, flags=re.IGNORECASE).strip()
        if stripped and stripped != text:
            return stripped
    return text


def _heuristic_plan(message: str) -> PlanResponse:
    lower = message.strip().lower()
    # #region agent log
    _debug_log(
        "parser.py:_heuristic_plan",
        "heuristic plan invoked",
        {
            "messagePreview": message[:120],
            "matchedTrigger": _matched_idea_trigger(message),
        },
        "H1",
    )
    # #endregion
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
    if _looks_like_idea(message):
        content = _extract_idea_content(message)
        tags = _infer_spark_tags(content.lower())
        return PlanResponse(
            intent="tool_use",
            tool="save_spark",
            tool_input=json.dumps({"content": content, "tags": tags}),
            reasoning="User shared an idea in natural language.",
            response=None,
        )
    fallback_response = _heuristic_chat_response(message)
    return PlanResponse(
        intent="chat",
        tool=None,
        tool_input=None,
        reasoning="General conversation fallback (MLX unavailable).",
        response=fallback_response,
    )


def _infer_spark_tags(text: str) -> list[str]:
    tags: list[str] = []
    if any(w in text for w in ("van", "camper", "roof rack", "solar panel")):
        tags.append("the_van")
    if any(w in text for w in ("land", "garden", "fence", "trees", "property")):
        tags.append("the_land")
    if any(w in text for w in ("trip", "travel", "road trip", "scotland", "journey")):
        tags.append("travelling")
    if any(w in text for w in ("project", "app", "build", "code")):
        tags.append("projects")
    if not tags:
        tags.append("general_life")
    return tags


def _heuristic_chat_response(message: str) -> str:
    text = message.strip()
    if not text:
        return "I'm here and listening. What would you like to work on?"
    lowered = text.lower()
    if "?" in text:
        if any(greet in lowered for greet in ("how are you", "how's it going", "hows it going")):
            return "Doing well and ready to help. What would you like to focus on?"
        return f"I got your question: \"{text}\". I can help sketch an answer while MLX reconnects."
    return f"Noted: \"{text}\". I can help you structure or refine this while full chat is reconnecting."


def parse_plan(raw: str, message: str) -> PlanResponse:
    try:
        data = _extract_json(raw)
        pref = data.get("preference_detected")
        dec = data.get("decision_detected")
        plan = PlanResponse(
            intent=data.get("intent", "chat"),
            tool=data.get("tool"),
            tool_input=data.get("tool_input"),
            reasoning=data.get("reasoning", ""),
            response=data.get("response"),
            task_state=data.get("task_state"),
            preference_detected=PreferenceDetected(**pref) if pref else None,
            decision_detected=DecisionDetected(**dec) if dec else None,
        )
        # #region agent log
        _debug_log(
            "parser.py:parse_plan",
            "mlx plan parsed",
            {
                "intent": plan.intent,
                "tool": plan.tool,
                "messagePreview": message[:120],
                "source": "mlx",
            },
            "H2",
        )
        # #endregion
        return plan
    except (json.JSONDecodeError, KeyError, TypeError) as e:
        # #region agent log
        _debug_log(
            "parser.py:parse_plan",
            "mlx json parse failed, using heuristic",
            {
                "error": type(e).__name__,
                "messagePreview": message[:120],
                "rawPreview": raw[:200] if raw else "",
            },
            "H2",
        )
        # #endregion
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
        if kind == "spark_archive":
            return {
                "summary": raw.strip() or "Deleted spark.",
                "topics": [],
                "key_facts": [],
            }
        return {"raw": raw.strip()}
