"""Goal conversation engine spanning Gates 1–4."""

from __future__ import annotations

import json
import re
import uuid
from datetime import datetime, timezone
from typing import Any

from .calendar import CalendarService
from .db import get_connection, init_db
from .schemas import ChatResponse, Goal, SessionOut, Spark, TodayResponse
from .sparks import SparkService


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
    m = re.search(r"(?:by\s+)?(?:the\s+)?end\s+of\s+(\w+)(?:\s+(\d{4}))?", lower)
    if m:
        month_name = m.group(1)
        year = m.group(2)
        month = MONTHS.get(month_name)
        if month:
            y = int(year) if year else datetime.now().year
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
            return grades[-1].upper()
        if len(grades) == 1 and not re.search(r"want|goal|by the end|aim", lower):
            return grades[0].upper()
    if domain == "reading":
        m = re.search(
            r"(?:currently|now|about)?\s*(\d+)\s*(?:books?|pages?)\s*(?:a|per)?\s*(?:week|month)?",
            lower,
        )
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


def _is_correction(text: str) -> bool:
    lower = text.lower()
    return any(
        p in lower
        for p in ("actually", "correction", "i meant", "change that", "instead", "update")
    )


def _is_approval(text: str) -> bool:
    lower = text.lower().strip()
    return any(
        p == lower or p in lower
        for p in (
            "yes",
            "yep",
            "yeah",
            "approve",
            "approved",
            "book it",
            "book them",
            "sounds good",
            "looks good",
            "go ahead",
            "that works",
            "confirm",
            "ok book",
            "okay book",
        )
    )


def _is_rejection(text: str) -> bool:
    lower = text.lower().strip()
    return any(
        p in lower
        for p in ("reject", "no thanks", "don't book", "do not book", "cancel proposal", "not those")
    )


def _is_plan_request(text: str) -> bool:
    lower = text.lower()
    return any(
        p in lower
        for p in (
            "look at the calendar",
            "check availability",
            "propose",
            "plan sessions",
            "schedule",
            "find times",
            "that direction",
            "feels right",
            "sounds right",
            "let's plan",
            "lets plan",
            "go ahead and plan",
        )
    ) or lower in {"yes", "yep", "yeah", "ok", "okay", "sure"}


def _wants_adjustment(text: str) -> bool:
    lower = text.lower()
    return any(
        p in lower
        for p in ("adjust", "change the plan", "reschedule", "too many", "fewer", "more sessions")
    )


def _split_mixed_parts(text: str) -> list[str]:
    # Preserve unresolved fragments from mixed day dumps.
    parts = re.split(r"(?:\n+|;|\band also\b|\balso\b|\bplus\b)", text, flags=re.I)
    return [p.strip(" -•\t") for p in parts if p and p.strip(" -•\t")]


def _is_spark_fragment(text: str) -> bool:
    lower = text.lower()
    if any(p in lower for p in ("spark:", "idea:", "maybe later", "not a commitment", "remind me someday")):
        return True
    if lower.startswith("maybe ") and not _infer_domain(text):
        return True
    return False


def _is_outcome_fragment(text: str) -> bool:
    lower = text.lower()
    return ("completed" in lower or "finished" in lower or "done" in lower or "missed" in lower) and (
        "session" in lower or "climb" in lower or "reading" in lower or "today" in lower or "yesterday" in lower
    )


def _is_goal_fragment(text: str) -> bool:
    lower = text.lower()
    if _infer_domain(text) and re.search(r"\bi want\b|\bi'?d like\b|goal|by the end", lower):
        return True
    if re.search(r"\bv\d+\b", lower) and ("want" in lower or "climb" in lower):
        return True
    return False


def _merge_answer_into_goal(goal: Goal, text: str) -> Goal:
    domain = goal.domain or _infer_domain(text)
    if domain and not goal.domain:
        goal.domain = domain

    correcting = _is_correction(text)
    lower = text.lower()

    target = _extract_target(text, goal.domain)
    if target:
        if correcting and ("target" in lower or "goal" in lower or "instead" in lower or "meant" in lower):
            goal.target = target
        elif goal.target and not goal.baseline and target != goal.target and not correcting:
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

    grades = re.findall(r"\b(v\d+)\b", text.lower())
    if goal.domain == "climbing" and grades:
        g = grades[0].upper()
        if correcting and ("target" in lower or "instead" in lower or "meant" in lower):
            goal.target = g
        elif goal.target and g != goal.target and not correcting:
            goal.baseline = g
        elif not goal.target:
            goal.target = g

    frequency = _extract_frequency(text)
    if frequency:
        goal.frequency = frequency

    # Commitment language
    if "commit" in lower or "can do" in lower or "i'll do" in lower or "i will" in lower:
        freq = _extract_frequency(text)
        if freq:
            goal.commitment = freq
        elif goal.frequency and not goal.commitment:
            goal.commitment = goal.frequency

    if goal.target or goal.deadline or goal.domain:
        goal.title = _title_from_parts(goal.domain, goal.target, goal.deadline, goal.title)

    if not _missing_slots(goal):
        if goal.status == "gathering":
            goal.status = "ready_to_plan"
    else:
        goal.status = "gathering"

    return goal


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
    if current.title and _infer_domain(text) and _infer_domain(text) != current.domain:
        if re.search(r"\bi want to\b|\bi'?d like to\b", lower):
            return True
    return False


def _format_session_line(s: SessionOut) -> str:
    start = datetime.fromisoformat(s.start_at)
    return f"- {start.strftime('%a %d %b %H:%M')}–{datetime.fromisoformat(s.end_at).strftime('%H:%M')} ({s.title})"


class GoalConversationService:
    def __init__(self, db_path=None):
        self.db_path = db_path
        self.conn = get_connection(db_path)
        init_db(self.conn)
        self.calendar = CalendarService(self.conn)
        self.sparks = SparkService(self.conn)

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

    def _create_goal_from_text(self, cid: str, text: str) -> Goal:
        domain = _infer_domain(text)
        target = _extract_target(text, domain)
        deadline = _extract_deadline(text)
        title = _title_from_parts(domain, target, deadline, text)
        baseline = _extract_baseline(text, domain)
        if target and baseline and baseline.upper() == target.upper():
            baseline = None
        goal = Goal(
            id=_new_id(),
            conversation_id=cid,
            title=title,
            domain=domain,
            target=target,
            deadline=deadline,
            baseline=baseline,
            frequency=_extract_frequency(text),
            status="gathering",
        )
        if not _missing_slots(goal):
            goal.status = "ready_to_plan"
        return goal

    def _compose_gather_reply(self, goal: Goal, just_created: bool) -> tuple[str, str | None]:
        missing = _missing_slots(goal)
        if missing:
            q = _question_for(goal, missing[0])
            if just_created:
                reply = f"Got it — I'll treat this as one goal: {goal.title}. {q}"
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

        summary_parts = [goal.title]
        if goal.baseline:
            summary_parts.append(f"you're at {goal.baseline}")
        if goal.frequency:
            summary_parts.append(f"you currently manage {goal.frequency}")
        if not goal.commitment:
            goal.commitment = goal.frequency
        reply = (
            "I've updated the same goal — "
            + "; ".join(summary_parts)
            + f". A practical commitment is {goal.commitment}. "
            "I can look at real calendar availability and propose dated sessions when you say the word."
        )
        return reply, None

    def _propose_for_goal(self, goal: Goal) -> tuple[str, list[SessionOut]]:
        if not goal.commitment and goal.frequency:
            goal.commitment = goal.frequency
            self._save_goal(goal)
        sessions = self.calendar.propose_goal_sessions(goal)
        if not sessions:
            return (
                "I checked availability around your fixed commitments and couldn't find open slots in the next few weeks. "
                "Want to loosen the commitment or free an evening?",
                [],
            )
        goal.status = "planned"
        self._save_goal(goal)
        lines = "\n".join(_format_session_line(s) for s in sessions[:12])
        more = "" if len(sessions) <= 12 else f"\n(+{len(sessions) - 12} more)"
        reply = (
            f"I found real availability for {goal.title} and proposed these sessions:\n{lines}{more}\n\n"
            "Approve to book each one once (linked to this goal), or reject and we'll adjust."
        )
        return reply, sessions

    def handle_message(self, message: str, conversation_id: str | None = None) -> ChatResponse:
        text = message.strip()
        cid = self._ensure_conversation(conversation_id)
        self._add_message(cid, "user", text)

        proposed: list[SessionOut] = []
        booked: list[SessionOut] = []
        captured_sparks: list[Spark] = []
        unresolved: list[str] = []
        goal = self._load_active_goal(cid)

        parts = _split_mixed_parts(text)
        mixed = len(parts) > 1

        if mixed:
            reply_chunks: list[str] = []
            for part in parts:
                if _is_spark_fragment(part):
                    spark = self.sparks.capture(re.sub(r"^(spark:|idea:)\s*", "", part, flags=re.I))
                    captured_sparks.append(spark)
                    reply_chunks.append(f"Saved spark (not a commitment): {spark.content}")
                    continue
                if _is_outcome_fragment(part) and goal:
                    session = self.calendar.find_session_for_outcome_phrase(part, goal.id)
                    if session:
                        outcome = "missed" if "missed" in part.lower() else "completed"
                        updated = self.calendar.mark_outcome(session["id"], outcome)
                        if updated:
                            reply_chunks.append(
                                f"Recorded {outcome} for {datetime.fromisoformat(updated.start_at).strftime('%a %d %b')}."
                            )
                            continue
                    unresolved.append(part)
                    reply_chunks.append(f"Kept unresolved for later: {part}")
                    continue
                if _is_goal_fragment(part) or (_infer_domain(part) and "want" in part.lower()):
                    if goal and not _is_interruption_new_goal(part, goal) and goal.status == "gathering":
                        goal = _merge_answer_into_goal(goal, part)
                        self._save_goal(goal)
                        chunk, _ = self._compose_gather_reply(goal, just_created=False)
                        reply_chunks.append(chunk)
                    else:
                        goal = self._create_goal_from_text(cid, part)
                        self._save_goal(goal)
                        chunk, _ = self._compose_gather_reply(goal, just_created=True)
                        reply_chunks.append(chunk)
                    continue
                # Follow-up only when the fragment clearly fills goal slots.
                if goal and goal.status == "gathering":
                    looks_like_answer = bool(
                        _extract_frequency(part)
                        or _extract_baseline(part, goal.domain)
                        or _extract_target(part, goal.domain)
                        or _is_correction(part)
                    )
                    if looks_like_answer:
                        goal = _merge_answer_into_goal(goal, part)
                        self._save_goal(goal)
                        chunk, _ = self._compose_gather_reply(goal, just_created=False)
                        reply_chunks.append(chunk)
                        continue
                unresolved.append(part)
                reply_chunks.append(f"Noted, left unresolved: {part}")
            reply = "\n".join(reply_chunks)
            self._add_message(cid, "assistant", reply)
            return ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                pending_question=None,
                proposed_sessions=proposed,
                booked_sessions=booked,
                sparks=captured_sparks,
                unresolved=unresolved,
            )

        # Single-intent path
        if _is_spark_fragment(text) and not _is_goal_fragment(text):
            spark = self.sparks.capture(re.sub(r"^(spark:|idea:)\s*", "", text, flags=re.I))
            reply = f"Saved as a spark — an idea, not a commitment: {spark.content}"
            self._add_message(cid, "assistant", reply)
            return ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                sparks=[spark],
            )

        # Approval / rejection of open proposal
        if goal:
            batch_id = self.calendar.open_proposal_batch(goal.id)
            if batch_id and _is_approval(text) and not _wants_adjustment(text):
                booked = self.calendar.approve_batch(batch_id)
                goal = self.get_goal(goal.id)
                reply = (
                    f"Booked {len(booked)} session(s) once each and linked them to {goal.title if goal else 'your goal'}. "
                    "Tell me when one is completed or missed and I'll record it."
                )
                self._add_message(cid, "assistant", reply)
                return ChatResponse(
                    conversation_id=cid,
                    reply=reply,
                    goal=goal,
                    booked_sessions=booked,
                )
            if batch_id and _is_rejection(text):
                n = self.calendar.reject_batch(batch_id)
                reply = f"Rejected {n} proposed session(s). Say if you want a different commitment or window and I'll propose again."
                self._add_message(cid, "assistant", reply)
                return ChatResponse(conversation_id=cid, reply=reply, goal=goal)

        # Outcomes
        if goal and _is_outcome_fragment(text):
            session = self.calendar.find_session_for_outcome_phrase(text, goal.id)
            if session:
                outcome = "missed" if "missed" in text.lower() else "completed"
                updated = self.calendar.mark_outcome(session["id"], outcome)
                progress = self.calendar.progress_for_goal(goal.id)
                adjust = ""
                if outcome == "missed" and _wants_adjustment(text):
                    adjust = " I can propose a replacement session if you want — you keep final control."
                elif outcome == "missed":
                    adjust = " If you want an adjustment, say the word and I'll propose options."
                reply = (
                    f"Recorded {outcome} against the session on "
                    f"{datetime.fromisoformat(session['start_at']).strftime('%a %d %b')}. "
                    f"Progress so far: {progress['completed']} completed, {progress['missed']} missed, "
                    f"{progress['scheduled']} still scheduled.{adjust}"
                )
                self._add_message(cid, "assistant", reply)
                return ChatResponse(conversation_id=cid, reply=reply, goal=goal)

        # Propose from ready goal
        if goal and goal.status in {"ready_to_plan", "planned", "active"} and _is_plan_request(text):
            if _wants_adjustment(text) and not _is_approval(text):
                reply = (
                    "Understood — I won't change booked sessions unless you approve a new proposal. "
                    "Tell me what to change (fewer nights, different hours), then I'll propose again."
                )
                self._add_message(cid, "assistant", reply)
                return ChatResponse(conversation_id=cid, reply=reply, goal=goal)
            reply, proposed = self._propose_for_goal(goal)
            self._add_message(cid, "assistant", reply)
            return ChatResponse(
                conversation_id=cid,
                reply=reply,
                goal=goal,
                proposed_sessions=proposed,
                pending_question="Approve or reject these proposed sessions?",
            )

        just_created = False
        if goal is None or _is_interruption_new_goal(text, goal):
            goal = self._create_goal_from_text(cid, text)
            just_created = True
        else:
            goal = _merge_answer_into_goal(goal, text)

        self._save_goal(goal)
        reply, pending = self._compose_gather_reply(goal, just_created=just_created)

        # Auto-offer planning nudge stays text-only; proposals require consent via plan request / yes.
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
        progress = []
        for g in goals:
            missing = _missing_slots(g)
            if missing:
                q = _question_for(g, missing[0])
                pending.append(q)
                attention.append(f"{g.title} still needs: {', '.join(missing)}")
            batch = self.calendar.open_proposal_batch(g.id)
            if batch:
                attention.append(f"{g.title} has sessions waiting for approval")
                pending.append("Approve or reject the proposed sessions?")
            prog = self.calendar.progress_for_goal(g.id)
            progress.append({"goal_id": g.id, "title": g.title, **prog})

        today = datetime.now().date().isoformat()
        todays = self.calendar.list_sessions(start=f"{today}T00:00:00", end=f"{today}T23:59:59")
        todays = [s for s in todays if s.status in {"scheduled", "completed", "missed", "proposed"}]
        return TodayResponse(
            goals=goals,
            attention=attention,
            pending_questions=pending,
            todays_sessions=todays,
            progress=progress,
        )

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
