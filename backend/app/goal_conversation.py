"""Goal conversation engine for Gate 1.

Understands unclear goals, asks only relevant follow-ups, and keeps one stable
goal identity across corrections, interruptions, and restarts.
"""

from __future__ import annotations

import json
import re
import uuid
from datetime import datetime, timezone
from typing import Any

from .db import get_connection, init_db
from .schemas import ChatResponse, Goal, TodayResponse


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _new_id() -> str:
    return str(uuid.uuid4())


MONTHS = {
    "january": 1,
    "february": 2,
    "march": 3,
    "april": 4,
    "may": 5,
    "june": 6,
    "july": 7,
    "august": 8,
    "september": 9,
    "october": 10,
    "november": 11,
    "december": 12,
}


def _infer_domain(text: str) -> str | None:
    lower = text.lower()
    if any(w in lower for w in ("climb", "climbing", "boulder", "v0", "v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8")):
        return "climbing"
    if any(w in lower for w in ("read", "reading", "book", "pages")):
        return "reading"
    if any(w in lower for w in ("save", "saving", "money", "£", "$", "budget")):
        return "savings"
    return None


def _extract_deadline(text: str) -> str | None:
    lower = text.lower()
    # end of <month> [year]
    m = re.search(r"(?:by\s+)?(?:the\s+)?end\s+of\s+(\w+)(?:\s+(\d{4}))?", lower)
    if m:
        month_name = m.group(1)
        year = m.group(2)
        month = MONTHS.get(month_name)
        if month:
            y = int(year) if year else datetime.now().year
            # if month already passed and no year, roll forward
            now = datetime.now()
            if not year and (month < now.month or (month == now.month and now.day > 28)):
                y = now.year + 1
            return f"{y:04d}-{month:02d}"
    m = re.search(r"by\s+(\w+)(?:\s+(\d{4}))?", lower)
    if m and m.group(1) in MONTHS:
        month = MONTHS[m.group(1)]
        year = m.group(2)
        y = int(year) if year else datetime.now().year
        now = datetime.now()
        if not year and month < now.month:
            y = now.year + 1
        return f"{y:04d}-{month:02d}"
    return None


def _extract_target(text: str, domain: str | None) -> str | None:
    lower = text.lower()
    if domain == "climbing" or re.search(r"\bv\d+\b", lower):
        m = re.search(r"\b(v\d+)\b", lower)
        if m:
            return m.group(1).upper()
    if domain == "reading" or "book" in lower or "read" in lower:
        m = re.search(r"(\d+)\s+books?", lower)
        if m:
            return f"{m.group(1)} books"
        m = re.search(r"(\d+)\s+pages?", lower)
        if m:
            return f"{m.group(1)} pages"
    if domain == "savings" or "save" in lower:
        m = re.search(r"(?:£|\$)\s?([\d,]+)", text)
        if m:
            return m.group(0).replace(" ", "")
        m = re.search(r"([\d,]+)\s*(?:pounds|dollars|quid)", lower)
        if m:
            return m.group(1)
    return None


def _extract_baseline(text: str, domain: str | None) -> str | None:
    lower = text.lower()
    if domain == "climbing" or re.search(r"\bv\d+\b", lower):
        grades = re.findall(r"\b(v\d+)\b", lower)
        if len(grades) >= 2:
            # "climb V6" + "V4" → baseline is the non-target if distinguishable later
            return grades[-1].upper()
        if len(grades) == 1 and not re.search(r"want|goal|by the end|aim", lower):
            return grades[0].upper()
    if domain == "reading":
        m = re.search(r"(?:currently|now|about)?\s*(\d+)\s*(?:books?|pages?)\s*(?:a|per)?\s*(?:week|month)?", lower)
        if m:
            return m.group(0).strip()
    if domain == "savings":
        m = re.search(r"(?:currently|now|have)\s*(?:saved\s*)?(?:£|\$)?\s*([\d,]+)", lower)
        if m:
            return m.group(0).strip()
    return None


def _extract_frequency(text: str) -> str | None:
    lower = text.lower().strip()
    patterns = [
        (r"\b(once|one)\s+(?:a|per)\s+week\b", "once a week"),
        (r"\b(twice|two\s+times)\s+(?:a|per)\s+week\b", "twice a week"),
        (r"\b(\d+)\s+times?\s+(?:a|per)\s+week\b", None),
        (r"\bevery\s+day\b", "every day"),
        (r"\bdaily\b", "every day"),
        (r"\b(\d+)\s+times?\s+(?:a|per)\s+month\b", None),
    ]
    for pattern, fixed in patterns:
        m = re.search(pattern, lower)
        if not m:
            continue
        if fixed:
            return fixed
        return m.group(0)
    # bare "twice a week" already covered; also "2x/week"
    m = re.search(r"\b(\d+)\s*x\s*/?\s*week\b", lower)
    if m:
        n = int(m.group(1))
        if n == 1:
            return "once a week"
        if n == 2:
            return "twice a week"
        return f"{n} times a week"
    return None


def _title_from_parts(domain: str | None, target: str | None, deadline: str | None, raw: str) -> str:
    if domain == "climbing" and target:
        base = f"Climb {target}"
    elif domain == "reading" and target:
        base = f"Read {target}"
    elif domain == "savings" and target:
        base = f"Save {target}"
    else:
        # Use a cleaned first sentence, not a shallow capture label.
        cleaned = re.sub(r"\s+", " ", raw.strip())
        cleaned = re.sub(r"^(i\s+want\s+to|i'?d\s+like\s+to|help\s+me)\s+", "", cleaned, flags=re.I)
        base = cleaned[:80].rstrip(".")
        base = base[0].upper() + base[1:] if base else "New goal"
    if deadline:
        try:
            year, month = deadline.split("-")
            month_name = list(MONTHS.keys())[int(month) - 1].capitalize()
            return f"{base} by end of {month_name}"
        except Exception:
            return base
    return base


def _missing_slots(goal: Goal) -> list[str]:
    missing: list[str] = []
    if not goal.baseline:
        missing.append("baseline")
    if not goal.frequency:
        missing.append("frequency")
    # commitment is discussed after baseline+frequency; not required for gate 1 second answer
    return missing


def _question_for(goal: Goal, slot: str) -> str:
    domain = goal.domain
    if slot == "baseline":
        if domain == "climbing":
            return "What grade do you climb consistently right now?"
        if domain == "reading":
            return "How much are you reading consistently right now?"
        if domain == "savings":
            return "How much have you already saved toward this, and what can you put aside now?"
        return "Where are you starting from with this today?"
    if slot == "frequency":
        if domain == "climbing":
            return "How often are you climbing each week right now?"
        if domain == "reading":
            return "How often can you read in a typical week?"
        if domain == "savings":
            return "How often can you set money aside?"
        return "How often can you work on this each week?"
    if slot == "commitment":
        return "What weekly commitment feels realistic before we look at the calendar?"
    return "What else should I know before we plan this?"


def _merge_answer_into_goal(goal: Goal, text: str) -> Goal:
    domain = goal.domain or _infer_domain(text)
    if domain and not goal.domain:
        goal.domain = domain

    correcting = _is_correction(text)
    lower = text.lower()

    target = _extract_target(text, goal.domain)
    if target:
        if correcting and (
            "target" in lower or "goal" in lower or "instead" in lower or "meant" in lower
        ):
            goal.target = target
        elif goal.target and not goal.baseline and target != goal.target and not correcting:
            # Answering current level while a target already exists.
            goal.baseline = target
        elif not goal.target:
            goal.target = target
        elif correcting:
            goal.target = target

    deadline = _extract_deadline(text)
    if deadline:
        goal.deadline = deadline

    baseline = _extract_baseline(text, goal.domain)
    if baseline and not (goal.target and baseline.upper() == goal.target.upper() and not goal.baseline):
        if not (goal.target and baseline.upper() == goal.target.upper()):
            goal.baseline = baseline

    # Climbing follow-up: "V4, twice a week" updates baseline when target already exists.
    grades = re.findall(r"\b(v\d+)\b", text.lower())
    if goal.domain == "climbing" and grades:
        g = grades[0].upper()
        if correcting and ("target" in lower or "instead" in lower or "meant" in lower):
            goal.target = g
        elif goal.target and g != goal.target and not correcting:
            if not goal.baseline:
                goal.baseline = g
            elif g != goal.baseline:
                goal.baseline = g
        elif not goal.target:
            goal.target = g

    frequency = _extract_frequency(text)
    if frequency:
        goal.frequency = frequency

    if goal.target or goal.deadline or goal.domain:
        goal.title = _title_from_parts(goal.domain, goal.target, goal.deadline, goal.title)

    if not _missing_slots(goal):
        goal.status = "ready_to_plan"
    else:
        goal.status = "gathering"

    return goal


def _is_correction(text: str) -> bool:
    lower = text.lower()
    return any(
        p in lower
        for p in (
            "actually",
            "correction",
            "i meant",
            "change that",
            "not ",
            "instead",
            "update",
        )
    )


def _is_interruption_new_goal(text: str, current: Goal | None) -> bool:
    if current is None:
        return True
    lower = text.lower()
    starts_new = any(
        p in lower
        for p in ("also i want", "another goal", "new goal", "separate goal", "meanwhile i want")
    )
    if starts_new:
        return True
    # Strong new-goal framing while current is already gathering something else
    if current.title and _infer_domain(text) and _infer_domain(text) != current.domain:
        if re.search(r"\bi want to\b|\bi'?d like to\b", lower):
            return True
    return False


class GoalConversationService:
    def __init__(self, db_path=None):
        self.db_path = db_path
        self.conn = get_connection(db_path)
        init_db(self.conn)

    def close(self) -> None:
        self.conn.close()

    def _ensure_conversation(self, conversation_id: str | None) -> str:
        if conversation_id:
            row = self.conn.execute(
                "SELECT id FROM conversations WHERE id = ?", (conversation_id,)
            ).fetchone()
            if row:
                self.conn.execute(
                    "UPDATE conversations SET updated_at = ? WHERE id = ?",
                    (_now(), conversation_id),
                )
                self.conn.commit()
                return conversation_id
        cid = _new_id()
        now = _now()
        self.conn.execute(
            "INSERT INTO conversations (id, created_at, updated_at) VALUES (?, ?, ?)",
            (cid, now, now),
        )
        self.conn.commit()
        return cid

    def _add_message(self, conversation_id: str, role: str, content: str) -> None:
        self.conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, created_at) VALUES (?, ?, ?, ?, ?)",
            (_new_id(), conversation_id, role, content, _now()),
        )
        self.conn.commit()

    def _load_active_goal(self, conversation_id: str) -> Goal | None:
        row = self.conn.execute(
            """
            SELECT * FROM goals
            WHERE conversation_id = ? AND status IN ('gathering', 'ready_to_plan', 'planned', 'active')
            ORDER BY updated_at DESC
            LIMIT 1
            """,
            (conversation_id,),
        ).fetchone()
        if not row:
            return None
        return self._row_to_goal(row)

    def _row_to_goal(self, row) -> Goal:
        return Goal(
            id=row["id"],
            conversation_id=row["conversation_id"],
            title=row["title"],
            domain=row["domain"],
            target=row["target"],
            deadline=row["deadline"],
            baseline=row["baseline"],
            frequency=row["frequency"],
            commitment=row["commitment"],
            status=row["status"],
            facts=json.loads(row["facts_json"] or "{}"),
        )

    def _save_goal(self, goal: Goal) -> None:
        now = _now()
        existing = self.conn.execute("SELECT id FROM goals WHERE id = ?", (goal.id,)).fetchone()
        payload = (
            goal.conversation_id,
            goal.title,
            goal.domain,
            goal.target,
            goal.deadline,
            goal.baseline,
            goal.frequency,
            goal.commitment,
            goal.status,
            json.dumps(goal.facts),
            now,
            goal.id,
        )
        if existing:
            self.conn.execute(
                """
                UPDATE goals SET
                    conversation_id=?, title=?, domain=?, target=?, deadline=?,
                    baseline=?, frequency=?, commitment=?, status=?, facts_json=?, updated_at=?
                WHERE id=?
                """,
                payload,
            )
        else:
            self.conn.execute(
                """
                INSERT INTO goals (
                    id, conversation_id, title, domain, target, deadline,
                    baseline, frequency, commitment, status, facts_json, created_at, updated_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                """,
                (
                    goal.id,
                    goal.conversation_id,
                    goal.title,
                    goal.domain,
                    goal.target,
                    goal.deadline,
                    goal.baseline,
                    goal.frequency,
                    goal.commitment,
                    goal.status,
                    json.dumps(goal.facts),
                    now,
                    now,
                ),
            )
        self.conn.commit()

    def _compose_reply(self, goal: Goal, just_created: bool) -> tuple[str, str | None]:
        missing = _missing_slots(goal)
        if missing:
            q = _question_for(goal, missing[0])
            if just_created:
                if goal.domain == "climbing" and goal.target:
                    reply = (
                        f"Got it — I'll treat this as one goal: {goal.title}. "
                        f"{q}"
                    )
                else:
                    reply = f"Understood. I'm tracking this as: {goal.title}. {q}"
            else:
                bits = []
                if goal.baseline:
                    bits.append(f"starting from {goal.baseline}")
                if goal.frequency:
                    bits.append(f"at {goal.frequency}")
                progress = ", ".join(bits)
                if progress:
                    reply = f"Updated the same goal ({goal.title}) with {progress}. {q}"
                else:
                    reply = f"Still working on {goal.title}. {q}"
            return reply, q

        # Enough to discuss commitment / readiness (Gate 1 ends after second answer)
        summary_parts = [goal.title]
        if goal.baseline:
            summary_parts.append(f"you're at {goal.baseline}")
        if goal.frequency:
            summary_parts.append(f"you currently manage {goal.frequency}")
        reply = (
            "I've updated the same goal — "
            + "; ".join(summary_parts)
            + ". A practical next step is to agree a weekly commitment, then look at real calendar availability. "
            "Does that direction feel right, or want to adjust anything first?"
        )
        return reply, None

    def handle_message(self, message: str, conversation_id: str | None = None) -> ChatResponse:
        text = message.strip()
        cid = self._ensure_conversation(conversation_id)
        self._add_message(cid, "user", text)

        goal = self._load_active_goal(cid)
        just_created = False

        if goal is None or _is_interruption_new_goal(text, goal):
            domain = _infer_domain(text)
            target = _extract_target(text, domain)
            deadline = _extract_deadline(text)
            title = _title_from_parts(domain, target, deadline, text)
            goal = Goal(
                id=_new_id(),
                conversation_id=cid,
                title=title,
                domain=domain,
                target=target,
                deadline=deadline,
                baseline=_extract_baseline(text, domain)
                if not (target and _extract_baseline(text, domain) == target)
                else None,
                frequency=_extract_frequency(text),
                status="gathering",
            )
            # Opening climb message: baseline extract may wrongly pick V6; clear if same as target
            if goal.target and goal.baseline and goal.baseline.upper() == goal.target.upper():
                goal.baseline = None
            just_created = True
        else:
            goal = _merge_answer_into_goal(goal, text)
            if _is_correction(text):
                # Keep same id; title refresh handled in merge
                pass

        self._save_goal(goal)
        reply, pending = self._compose_reply(goal, just_created=just_created)
        self._add_message(cid, "assistant", reply)

        return ChatResponse(
            conversation_id=cid,
            reply=reply,
            goal=goal,
            pending_question=pending,
        )

    def get_today(self) -> TodayResponse:
        rows = self.conn.execute(
            """
            SELECT * FROM goals
            WHERE status IN ('gathering', 'ready_to_plan', 'planned', 'active')
            ORDER BY updated_at DESC
            """
        ).fetchall()
        goals = [self._row_to_goal(r) for r in rows]
        attention: list[str] = []
        pending: list[str] = []
        for g in goals:
            missing = _missing_slots(g)
            if missing:
                q = _question_for(g, missing[0])
                pending.append(q)
                attention.append(f"{g.title} still needs: {', '.join(missing)}")
        return TodayResponse(goals=goals, attention=attention, pending_questions=pending)

    def get_goal(self, goal_id: str) -> Goal | None:
        row = self.conn.execute("SELECT * FROM goals WHERE id = ?", (goal_id,)).fetchone()
        return self._row_to_goal(row) if row else None

    def list_messages(self, conversation_id: str) -> list[dict[str, Any]]:
        rows = self.conn.execute(
            """
            SELECT id, conversation_id, role, content, created_at
            FROM messages WHERE conversation_id = ?
            ORDER BY created_at ASC
            """,
            (conversation_id,),
        ).fetchall()
        return [dict(r) for r in rows]
