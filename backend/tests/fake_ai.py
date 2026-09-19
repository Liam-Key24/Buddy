"""Scripted Cloud AI for tests — never hits the real Groq API."""

from __future__ import annotations

import json
import re
from typing import Any


class FakeGroq:
    """Deterministic BuddyTurn factory driven by the latest user message in the payload."""

    def __init__(self):
        self.calls = 0
        self.closed = False

    def close(self) -> None:
        self.closed = True

    def complete_json(self, system: str, user: str, *, allow_retry: bool = True) -> dict[str, Any]:
        self.calls += 1
        payload = json.loads(user)
        message = payload["user_message"]
        active = payload.get("active_goal")
        lower = message.lower()

        # Day dump / spark
        if "spark:" in lower or lower.startswith("idea:"):
            content = re.sub(r"^(spark:|idea:)\s*", "", message, flags=re.I).strip()
            return {
                "assistant_text": f"Saved spark (not a commitment): {content}",
                "intents": ["spark_capture"],
                "goal_updates": [],
                "clarification": None,
                "requested_action": {"type": "none", "spark_content": content},
                "confidence": 0.9,
            }

        # Product / Mevero
        if "mevero" in lower or ("product" in lower and "want" in lower):
            return {
                "assistant_text": (
                    "Got it — Finish Mevero by end of December"
                    + (" (about 80% done)" if "80%" in lower or "80" in lower else "")
                    + ". How often can you work on shipping this each week?"
                ),
                "intents": ["goal_create"],
                "goal_updates": [
                    {
                        "action": "create",
                        "title": "Finish Mevero by end of December",
                        "domain": "product",
                        "target": "Mevero",
                        "deadline": "2026-12",
                        "baseline": "about 80% done" if "80" in lower else None,
                        "status": "gathering",
                        "facts": {"progress_pct": 80} if "80" in lower else {},
                    }
                ],
                "clarification": "How often can you work on shipping this each week?",
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        # Climbing create
        if "climb" in lower and "v6" in lower and (active is None or "climb" not in (active.get("title") or "").lower()):
            return {
                "assistant_text": (
                    "Got it — Climb V6 by end of November. "
                    "What grade do you climb consistently right now?"
                ),
                "intents": ["goal_create"],
                "goal_updates": [
                    {
                        "action": "create",
                        "title": "Climb V6 by end of November",
                        "domain": "climbing",
                        "target": "V6",
                        "deadline": "2026-11",
                        "status": "gathering",
                        "facts": {},
                    }
                ],
                "clarification": "What grade do you climb consistently right now?",
                "requested_action": {"type": "none"},
                "confidence": 0.92,
            }

        # Climbing follow-up / correction
        if active and active.get("domain") == "climbing":
            if "v7" in lower and ("instead" in lower or "actually" in lower or "target" in lower):
                return {
                    "assistant_text": "Updated the same goal — target is now V7.",
                    "intents": ["goal_update"],
                    "goal_updates": [{"action": "update", "target": "V7", "title": "Climb V7 by end of November"}],
                    "clarification": None,
                    "requested_action": {"type": "none"},
                    "confidence": 0.9,
                }
            if re.search(r"\bv4\b", lower) or "twice" in lower:
                return {
                    "assistant_text": (
                        "Updated the same goal (Climb V6 by end of November) with V4, twice a week. "
                        "That cadence looks workable — want me to check the calendar and propose sessions?"
                    ),
                    "intents": ["goal_update"],
                    "goal_updates": [
                        {
                            "action": "update",
                            "baseline": "V4",
                            "frequency": "twice a week",
                            "commitment": "twice a week",
                            "status": "ready_to_plan",
                        }
                    ],
                    "clarification": "Shall I propose sessions on the calendar?",
                    "requested_action": {"type": "none"},
                    "confidence": 0.93,
                }

        # Reading
        if "read" in lower and "book" in lower:
            return {
                "assistant_text": "Got it — Read 12 books by end of December. How much are you reading consistently right now?",
                "intents": ["goal_create"],
                "goal_updates": [
                    {
                        "action": "create",
                        "title": "Read 12 books by end of December",
                        "domain": "reading",
                        "target": "12 books",
                        "deadline": "2026-12",
                        "status": "gathering",
                    }
                ],
                "clarification": "How much are you reading consistently right now?",
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        if active and active.get("domain") == "reading" and ("week" in lower or "book" in lower):
            return {
                "assistant_text": "Updated the same reading goal with your current cadence.",
                "intents": ["goal_update"],
                "goal_updates": [
                    {
                        "action": "update",
                        "baseline": "About one book a month",
                        "frequency": "twice a week",
                        "status": "ready_to_plan",
                    }
                ],
                "clarification": None,
                "requested_action": {"type": "none"},
                "confidence": 0.88,
            }

        # Savings
        if "save" in lower and ("£" in message or "pound" in lower or re.search(r"\d{3,}", lower)):
            target = None
            m = re.search(r"£\s*([\d,]+)", message)
            if m:
                target = f"£{m.group(1).replace(',', '')}"
            return {
                "assistant_text": f"Got it — Save {target or 'toward your target'}. What remains after essentials?",
                "intents": ["goal_create"],
                "goal_updates": [
                    {
                        "action": "create",
                        "title": f"Save {target}" if target else "Savings goal",
                        "domain": "savings",
                        "target": target,
                        "deadline": "2026-03" if "march" in lower else ("2027-05" if "may" in lower else None),
                        "status": "gathering",
                    }
                ],
                "clarification": "What usually remains after essentials?",
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        if active and active.get("domain") == "savings":
            if "once a week" in lower or "400" in lower:
                return {
                    "assistant_text": "Updated the same savings goal.",
                    "intents": ["goal_update"],
                    "goal_updates": [
                        {
                            "action": "update",
                            "baseline": "£400 so far",
                            "frequency": "once a week",
                            "status": "ready_to_plan",
                            "facts": {"saved_so_far": 400},
                        }
                    ],
                    "clarification": None,
                    "requested_action": {"type": "none"},
                    "confidence": 0.9,
                }
            if "800" in lower or "leftover" in lower or "left" in lower:
                return {
                    "assistant_text": "Understood — with £800–£900 left and wanting spending money, ~£400/month on payday is a practical start.",
                    "intents": ["goal_update"],
                    "goal_updates": [
                        {
                            "action": "update",
                            "frequency": "once a month",
                            "commitment": "around £400 on payday",
                            "status": "ready_to_plan",
                            "facts": {
                                "monthly_leftover_low": 800,
                                "monthly_leftover_high": 900,
                                "wants_spending_buffer": True,
                            },
                        }
                    ],
                    "clarification": "Propose payday sessions, or choose a monthly amount first?",
                    "requested_action": {"type": "none"},
                    "confidence": 0.9,
                }
            if "how much can i save" in lower:
                return {
                    "assistant_text": (
                        "From the £800–£900 left after expenses, a realistic save is about £320–£540 each payday. "
                        "If that feels right, I can check your calendar and propose payday sessions for you to approve."
                    ),
                    "intents": ["goal_update"],
                    "goal_updates": [{"action": "update", "status": "ready_to_plan", "frequency": "once a month"}],
                    "clarification": "Shall I propose payday sessions on the calendar?",
                    "requested_action": {"type": "none"},
                    "confidence": 0.91,
                }

        calendar_sessions = payload.get("calendar_sessions") or []

        if "delete" in lower and calendar_sessions:
            if "all proposed" in lower or "proposed sessions" in lower:
                return {
                    "assistant_text": "Removed the proposed sessions from your calendar.",
                    "intents": ["calendar_delete"],
                    "goal_updates": [],
                    "calendar_actions": [
                        {
                            "op": "delete",
                            "statuses": ["proposed"],
                            "all_matching": True,
                        }
                    ],
                    "requested_action": {"type": "none"},
                    "confidence": 0.92,
                }
            target = calendar_sessions[0]
            return {
                "assistant_text": f"Removed {target['title']} from your calendar.",
                "intents": ["calendar_delete"],
                "goal_updates": [],
                "calendar_actions": [{"op": "delete", "session_id": target["id"]}],
                "requested_action": {"type": "none"},
                "confidence": 0.92,
            }

        if "rename" in lower and calendar_sessions:
            target = calendar_sessions[0]
            new_title = "Updated climb session"
            if "to " in lower:
                new_title = lower.split("to ", 1)[1].strip().title() or new_title
            return {
                "assistant_text": f"Renamed {target['title']} to {new_title}.",
                "intents": ["calendar_update"],
                "goal_updates": [],
                "calendar_actions": [
                    {
                        "op": "update",
                        "session_id": target["id"],
                        "new_title": new_title,
                    }
                ],
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        if ("move" in lower or "reschedule" in lower) and calendar_sessions:
            target = calendar_sessions[0]
            return {
                "assistant_text": f"Moved {target['title']} to Friday at 6pm.",
                "intents": ["calendar_move"],
                "goal_updates": [],
                "calendar_actions": [
                    {
                        "op": "move",
                        "session_id": target["id"],
                        "new_start_at": "2026-12-05T18:00:00+00:00",
                        "new_end_at": "2026-12-05T19:30:00+00:00",
                    }
                ],
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        if ("completed" in lower or "missed" in lower) and calendar_sessions:
            target = calendar_sessions[0]
            outcome = "missed" if "missed" in lower else "completed"
            return {
                "assistant_text": f"Marked {target['title']} as {outcome}.",
                "intents": ["session_outcome"],
                "goal_updates": [],
                "calendar_actions": [
                    {"op": "mark_outcome", "session_id": target["id"], "outcome": outcome}
                ],
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        # Plan request
        if any(
            p in lower
            for p in ("calendar", "propose", "schedule", "look at", "payday sessions")
        ):
            return {
                "assistant_text": "I'll check real availability and propose dated sessions for your approval.",
                "intents": ["goal_plan_request"],
                "goal_updates": [{"action": "update", "status": "ready_to_plan"}] if active else [],
                "clarification": "Approve or reject these proposed sessions?",
                "requested_action": {"type": "propose_sessions"},
                "confidence": 0.95,
            }

        # Product follow-up cadence
        if active and active.get("domain") == "product" and ("week" in lower or "hour" in lower):
            return {
                "assistant_text": "Updated the same goal with your shipping cadence. Ready to propose calendar sessions when you are.",
                "intents": ["goal_update"],
                "goal_updates": [
                    {
                        "action": "update",
                        "frequency": "three times a week" if "three" in lower else "twice a week",
                        "status": "ready_to_plan",
                    }
                ],
                "clarification": None,
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }

        # Default chat
        title = (active or {}).get("title") or "your goal"
        return {
            "assistant_text": f"I'm with you on {title}. What would help next?",
            "intents": ["chat"],
            "goal_updates": [],
            "clarification": None,
            "requested_action": {"type": "none"},
            "confidence": 0.5,
        }
