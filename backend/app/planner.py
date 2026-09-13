"""Conversation planner: understand goals, plan with the user, never book alone.

Prefers the local Qwen model. Falls back to a non-wall heuristic planner so
Buddy still reasons about capacity and next steps when the model is cold.
"""

from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any

from .schemas import Goal

SYSTEM = """You are Buddy, a local planning partner.
Product truth: turn unclear goals into realistic calendar actions, track what happens,
and adjust through conversation. The user retains final control.

Rules:
- Keep ONE stable goal unless the user clearly starts a separate new goal.
- Update the goal from natural language (money left over, payday, grades, etc.).
- Answer questions like "how much can I save?" using known facts; do not stonewall.
- Ask at most one useful question when something important is missing.
- Never claim sessions are booked. Only suggest proposing calendar times.
- When enough is agreed for scheduling, set ready_to_propose true and say you can
  check the calendar for dated sessions once they approve proposing.
- Return ONLY a JSON object (no markdown) with keys:
  reply (string),
  goal_patch (object with any of: title, domain, target, deadline, baseline,
    frequency, commitment, status, facts),
  ready_to_propose (boolean),
  pending_question (string|null)
"""


@dataclass
class PlanResult:
    reply: str
    goal_patch: dict[str, Any] = field(default_factory=dict)
    ready_to_propose: bool = False
    pending_question: str | None = None
    used_model: bool = False


def _goal_public(goal: Goal | None) -> dict[str, Any] | None:
    if goal is None:
        return None
    return goal.model_dump()


def _parse_json_object(text: str) -> dict[str, Any] | None:
    text = text.strip()
    if text.startswith("```"):
        text = re.sub(r"^```(?:json)?\s*", "", text)
        text = re.sub(r"\s*```$", "", text)
    try:
        data = json.loads(text)
        return data if isinstance(data, dict) else None
    except json.JSONDecodeError:
        pass
    m = re.search(r"\{.*\}", text, flags=re.S)
    if not m:
        return None
    try:
        data = json.loads(m.group(0))
        return data if isinstance(data, dict) else None
    except json.JSONDecodeError:
        return None


def _model_enabled() -> bool:
    # Opt-in: MLX can abort the process on unsupported Python/Metal setups.
    # Heuristic planner remains the default path; set BUDDY_USE_MLX=1 to try Qwen.
    return os.environ.get("BUDDY_USE_MLX", "").strip().lower() in {"1", "true", "yes", "on"}


def plan_with_model(goal: Goal | None, message: str, recent: list[dict[str, str]]) -> PlanResult | None:
    if not _model_enabled():
        return None
    from .mlx_model import generate_reply, load_model

    if not load_model():
        return None
    payload = {
        "current_goal": _goal_public(goal),
        "recent_messages": recent[-8:],
        "user_message": message,
    }
    raw = generate_reply(
        SYSTEM,
        json.dumps(payload, ensure_ascii=False),
        max_tokens=512,
    )
    if not raw:
        return None
    data = _parse_json_object(raw)
    if not data or not isinstance(data.get("reply"), str):
        # Model replied in prose — still usable as reply, no patch.
        return PlanResult(reply=raw.strip(), used_model=True)
    patch = data.get("goal_patch") or {}
    if not isinstance(patch, dict):
        patch = {}
    return PlanResult(
        reply=data["reply"].strip(),
        goal_patch=patch,
        ready_to_propose=bool(data.get("ready_to_propose")),
        pending_question=data.get("pending_question"),
        used_model=True,
    )


def _money_amounts(text: str) -> list[int]:
    found: list[int] = []
    for m in re.finditer(r"(?:£|\$)?\s*([\d]{1,3}(?:,\d{3})*|\d+)(?:\s*(?:pounds|quid))?", text, re.I):
        raw = m.group(1).replace(",", "")
        try:
            found.append(int(raw))
        except ValueError:
            continue
    return found


def _extract_iso_deadline(text: str) -> str | None:
    lower = text.lower()
    # 1 May 2027 / 1st May 2027 / May 1 2027
    months = {
        "jan": 1, "january": 1, "feb": 2, "february": 2, "mar": 3, "march": 3,
        "apr": 4, "april": 4, "may": 5, "jun": 6, "june": 6, "jul": 7, "july": 7,
        "aug": 8, "august": 8, "sep": 9, "sept": 9, "september": 9,
        "oct": 10, "october": 10, "nov": 11, "november": 11, "dec": 12, "december": 12,
    }
    m = re.search(
        r"\b(\d{1,2})(?:st|nd|rd|th)?\s+(january|february|march|april|may|june|july|august|september|october|november|december|jan|feb|mar|apr|jun|jul|aug|sep|sept|oct|nov|dec)\s+(\d{4})\b",
        lower,
    )
    if m:
        day, mon, year = int(m.group(1)), months[m.group(2)], int(m.group(3))
        return f"{year:04d}-{mon:02d}-{day:02d}"
    m = re.search(
        r"\b(january|february|march|april|may|june|july|august|september|october|november|december)\s+(\d{1,2})(?:st|nd|rd|th)?(?:\s*,?\s*(\d{4}))\b",
        lower,
    )
    if m:
        mon, day, year = months[m.group(1)], int(m.group(2)), int(m.group(3))
        return f"{year:04d}-{mon:02d}-{day:02d}"
    m = re.search(r"\bby\s+(\d{4})-(\d{2})(?:-(\d{2}))?\b", lower)
    if m:
        return f"{m.group(1)}-{m.group(2)}-{m.group(3) or '01'}"
    return None


def _months_until(deadline: str | None) -> int | None:
    if not deadline:
        return None
    try:
        parts = [int(x) for x in deadline.split("-")]
        year, month = parts[0], parts[1]
        day = parts[2] if len(parts) > 2 else 1
        end = datetime(year, month, day)
        now = datetime.now()
        months = (end.year - now.year) * 12 + (end.month - now.month)
        if end.day < now.day:
            months -= 1
        return max(1, months)
    except Exception:
        return None


def plan_heuristic(goal: Goal | None, message: str) -> PlanResult:
    """Non-wall planner: reason with known facts; don't re-ask the same slot blindly."""
    text = message.strip()
    lower = text.lower()
    patch: dict[str, Any] = {"facts": dict(goal.facts) if goal else {}}
    facts: dict[str, Any] = patch["facts"]

    # Climbing: target grade in the goal statement is not the baseline.
    if goal and goal.domain == "climbing":
        is_goal_statement = bool(re.search(r"\bwant to\b|\bby the end\b|\bgoal\b", lower))
        has_freq_answer = bool(re.search(r"week|daily|once|twice|\d+\s*x", lower))
        grades_in_msg = re.findall(r"\bv\d+\b", lower)
        target_grade = None
        if goal.target:
            tm = re.search(r"v\d+", str(goal.target).lower())
            if tm:
                target_grade = tm.group(0)
        # Follow-up baseline if a grade differs from target, or short answer after the goal exists.
        answering_baseline = False
        if grades_in_msg and not is_goal_statement:
            if any(g != target_grade for g in grades_in_msg):
                answering_baseline = True
            elif len(text) < 80:
                answering_baseline = True

        if not goal.baseline and not answering_baseline:
            return PlanResult(
                reply=(
                    f"Got it — one goal: {goal.title}. "
                    "What grade do you climb consistently right now?"
                ),
                goal_patch=patch,
                pending_question="What grade do you climb consistently right now?",
            )
        if (goal.baseline or answering_baseline) and not goal.frequency and not has_freq_answer:
            return PlanResult(
                reply=(
                    f"Updated the same goal ({goal.title})"
                    f"{f' starting from {goal.baseline}' if goal.baseline else ''}. "
                    "How often are you climbing each week right now?"
                ),
                goal_patch=patch,
                pending_question="How often are you climbing each week right now?",
            )
        if answering_baseline or has_freq_answer:
            return PlanResult(
                reply=(
                    f"Updated the same goal ({goal.title}) with what you just shared. "
                    "When you're ready I can check the calendar and propose sessions for your approval."
                ),
                goal_patch=patch,
            )

    # Product / shipping: keep planning domain-neutral (no leftover-money defaults).
    if goal and goal.domain == "product":
        is_goal_statement = bool(
            re.search(r"\bwant to\b|\bby the end\b|\bgoal\b|\bmake\b|\bbuild\b|\bship\b", lower)
        )
        has_freq_answer = bool(re.search(r"week|daily|once|twice|\d+\s*x", lower))
        pct = re.search(r"(\d+)\s*%", lower)
        if pct and "baseline" not in patch:
            patch["baseline"] = f"{pct.group(1)}% done"
        named = re.search(
            r"(?:product|app|saas|tool|startup|project)\s+called\s+([A-Za-z][\w-]*)",
            text,
            flags=re.I,
        )
        if named:
            patch.setdefault("domain", "product")
            patch.setdefault("target", named.group(1))
            if not (goal and goal.title):
                patch.setdefault("title", f"Finish {named.group(1)}")

        baseline = patch.get("baseline") or goal.baseline
        frequency = patch.get("frequency") or goal.frequency
        title = (goal.title if goal and goal.title else None) or patch.get("title") or "your product"
        if not baseline and is_goal_statement:
            return PlanResult(
                reply=(
                    f"Got it — one goal: {title}. "
                    "Where are you with this today — what's already done, and what's left?"
                ),
                goal_patch=patch,
                pending_question="Where are you with this today — what's already done, and what's left?",
            )
        if (baseline or pct) and not frequency and not has_freq_answer:
            shown_baseline = baseline or patch.get("baseline")
            about = f" (about {shown_baseline})" if shown_baseline else ""
            return PlanResult(
                reply=(
                    f"Got it — {title}{about}. "
                    "How often can you work on shipping this each week?"
                ),
                goal_patch=patch,
                pending_question="How often can you work on shipping this each week?",
            )
        if has_freq_answer:
            patch.setdefault("status", "ready_to_plan")
            return PlanResult(
                reply=(
                    f"Updated the same goal ({title}) with what you just shared. "
                    "When you're ready I can check the calendar and propose sessions for your approval."
                ),
                goal_patch=patch,
                ready_to_propose=True,
                pending_question="Shall I propose sessions on the calendar?",
            )

    # Seed goal fields from message
    if re.search(r"save|saving|£|\$", lower):
        patch.setdefault("domain", "savings")
    if re.search(r"\b(product|app|saas|mvp|startup)\b|\b(make|build|ship)\b.{0,40}\b(product|app)\b", lower):
        patch.setdefault("domain", "product")
    amounts = _money_amounts(text)
    if "save" in lower and amounts:
        # First large amount near save → target
        for a in amounts:
            if a >= 500:
                patch["target"] = f"£{a}"
                break
    if re.search(r"have\s+(?:about\s+|around\s+)?(?:£|\$)?\s*[\d,]+|[\d,]+\s+so far", lower):
        for a in amounts:
            if a < 5000:
                patch["baseline"] = f"£{a} saved so far"
                facts["saved_so_far"] = a
                break
    dl = _extract_iso_deadline(text)
    if dl:
        patch["deadline"] = dl

    if "paid" in lower or "payday" in lower or "last day of" in lower:
        facts["payday"] = "last day of month"
        patch["frequency"] = patch.get("frequency") or "once a month"
        patch["commitment"] = patch.get("commitment") or "once a month on payday"

    # Leftover / expenses language
    if "left over" in lower or "leftover" in lower or "left" in lower:
        # Prefer 800-900 style ranges
        m = re.search(r"(\d{2,4})\s*[-–]\s*(\d{2,4})", text)
        if m:
            lo, hi = int(m.group(1)), int(m.group(2))
            facts["monthly_leftover_low"] = lo
            facts["monthly_leftover_high"] = hi
            facts["monthly_leftover"] = (lo + hi) // 2
        elif amounts:
            facts["monthly_leftover"] = amounts[-1]
    if "expenses" in lower and amounts:
        facts["monthly_expenses"] = amounts[0] if amounts[0] < 5000 else amounts[-1]
    if "spending money" in lower or "buy different things" in lower:
        facts["wants_spending_buffer"] = True

    title_bits = []
    target = patch.get("target") or (goal.target if goal else None)
    if patch.get("domain") == "savings" or (goal and goal.domain == "savings"):
        if target:
            title_bits.append(f"Save {target}")
        deadline = patch.get("deadline") or (goal.deadline if goal else None)
        if deadline:
            title_bits.append(f"by {deadline}")
        if title_bits:
            patch["title"] = " ".join(title_bits)

    # Merge view of facts after patch
    saved = facts.get("saved_so_far")
    if saved is None and goal and goal.facts:
        saved = goal.facts.get("saved_so_far")
    leftover = facts.get("monthly_leftover")
    if leftover is None and goal and goal.facts:
        leftover = goal.facts.get("monthly_leftover")
    leftover_low = facts.get("monthly_leftover_low") or (goal.facts.get("monthly_leftover_low") if goal else None)
    leftover_high = facts.get("monthly_leftover_high") or (goal.facts.get("monthly_leftover_high") if goal else None)
    wants_buffer = facts.get("wants_spending_buffer") or (goal.facts.get("wants_spending_buffer") if goal else False)
    deadline = patch.get("deadline") or (goal.deadline if goal else None)
    target_s = patch.get("target") or (goal.target if goal else None)
    target_n = None
    if target_s:
        m = re.search(r"(\d+)", str(target_s).replace(",", ""))
        if m:
            target_n = int(m.group(1))

    # Answer "how much can I save?"
    if re.search(r"how much (can|could|should)?\s*(i|we)?\s*save", lower) or lower.strip() in {
        "how much can i save?",
        "how much can i save",
    }:
        if leftover or (leftover_low and leftover_high):
            lo = leftover_low or leftover
            hi = leftover_high or leftover
            # Keep a spending buffer: suggest ~half to 2/3 of leftover
            if wants_buffer:
                suggest_lo = max(50, int(lo * 0.4))
                suggest_hi = max(suggest_lo, int(hi * 0.6))
            else:
                suggest_lo = max(50, int(lo * 0.7))
                suggest_hi = max(suggest_lo, int(hi * 0.85))
            facts["suggested_monthly_save_low"] = suggest_lo
            facts["suggested_monthly_save_high"] = suggest_hi
            patch["commitment"] = f"£{suggest_lo}–£{suggest_hi} once a month on payday"
            patch["frequency"] = "once a month"
            patch["status"] = "ready_to_plan"
            months = _months_until(deadline)
            extra = ""
            if months and target_n and saved is not None:
                need = max(0, target_n - int(saved))
                per = (suggest_lo + suggest_hi) // 2
                months_needed = max(1, (need + per - 1) // per)
                extra = (
                    f" You still need about £{need}. At ~£{per}/month that's roughly "
                    f"{months_needed} months"
                )
                if months:
                    extra += f" (you have ~{months} months until {deadline})"
                extra += "."
                if months_needed <= months:
                    extra += " That pace can hit the goal if you stick with it."
                else:
                    extra += " We may need a higher monthly amount or a later deadline — your call."
            reply = (
                f"From the £{lo}–£{hi} left after expenses, and keeping spending money, "
                f"a realistic save is about £{suggest_lo}–£{suggest_hi} each payday"
                f"{' (last day of the month)' if facts.get('payday') or (goal and goal.facts.get('payday')) else ''}."
                f"{extra} "
                "If that feels right, I can check your calendar and propose payday save sessions for you to approve."
            )
            return PlanResult(
                reply=reply,
                goal_patch=patch,
                ready_to_propose=True,
                pending_question="Shall I propose payday sessions on the calendar?",
            )
        return PlanResult(
            reply=(
                "I can estimate once I know what you usually have left after essentials and "
                "how much spending money you want to keep. Rough monthly leftover?"
            ),
            goal_patch=patch,
            pending_question="What do you usually have left after essentials?",
        )

    # Opening / update with rich money context without frequency wall
    if leftover or leftover_low:
        lo = leftover_low or leftover
        hi = leftover_high or leftover
        if wants_buffer and not re.search(r"how much", lower):
            suggest = max(50, int(((lo or 0) + (hi or 0)) / 2 * 0.5))
            facts["suggested_monthly_save"] = suggest
            patch["frequency"] = patch.get("frequency") or "once a month"
            patch["commitment"] = patch.get("commitment") or f"around £{suggest} on payday"
            patch["status"] = "ready_to_plan"
            reply = (
                f"Understood — same goal stays in place"
                f"{f' ({patch.get('title') or (goal.title if goal else 'your savings goal')})'}. "
                f"With ~£{lo}–£{hi} left and wanting spending money, something like £{suggest}/month on payday "
                "is a practical starting commitment (not locked in). "
                "Want me to check the calendar and propose payday sessions for approval, "
                "or should we pick a different monthly amount first?"
            )
            return PlanResult(
                reply=reply,
                goal_patch=patch,
                ready_to_propose=False,
                pending_question="Propose payday sessions, or choose a monthly amount first?",
            )

    # New goal / first message
    if goal is None or patch.get("target") or patch.get("title"):
        title = patch.get("title") or (goal.title if goal else "your goal")
        domain = patch.get("domain") or (goal.domain if goal else None)
        if not goal:
            patch["status"] = "gathering"

        # Climbing: ask the useful next thing without becoming a generic wall later.
        if domain == "climbing":
            baseline = patch.get("baseline") or (goal.baseline if goal else None)
            frequency = patch.get("frequency") or (goal.frequency if goal else None)
            if not baseline:
                return PlanResult(
                    reply=(
                        f"Got it — one goal: {title}. "
                        "What grade do you climb consistently right now?"
                    ),
                    goal_patch=patch,
                    pending_question="What grade do you climb consistently right now?",
                )
            if not frequency:
                return PlanResult(
                    reply=(
                        f"Updated the same goal ({title})"
                        f"{f' starting from {baseline}' if baseline else ''}. "
                        "How often are you climbing each week right now?"
                    ),
                    goal_patch=patch,
                    pending_question="How often are you climbing each week right now?",
                )

        if domain == "product":
            baseline = patch.get("baseline") or (goal.baseline if goal else None)
            frequency = patch.get("frequency") or (goal.frequency if goal else None)
            if not baseline:
                return PlanResult(
                    reply=(
                        f"Got it — one goal: {title}. "
                        "Where are you with this today — what's already done, and what's left?"
                    ),
                    goal_patch=patch,
                    pending_question="Where are you with this today — what's already done, and what's left?",
                )
            if not frequency:
                about = f" (about {baseline})" if baseline else ""
                return PlanResult(
                    reply=(
                        f"Got it — {title}{about}. "
                        "How often can you work on shipping this each week?"
                    ),
                    goal_patch=patch,
                    pending_question="How often can you work on shipping this each week?",
                )

        has_payday = bool(facts.get("payday"))
        has_saved = saved is not None or "baseline" in patch
        if has_saved and has_payday:
            reply = (
                f"Got it — one goal: {title}. "
                f"I'll remember you're paid at month-end"
                f"{f' and have {patch.get('baseline', 'some savings')} already' if has_saved else ''}. "
                "What usually remains after essentials, and how much spending money do you want to keep? "
                "Then I can suggest a realistic monthly save and propose calendar sessions for your approval."
            )
            return PlanResult(reply=reply, goal_patch=patch, pending_question="What remains after essentials?")
        if patch.get("target") or (goal is None) or domain == "product":
            if domain == "savings":
                cadence_hint = "money left over, payday, how much you want to keep for spending"
            elif domain == "climbing":
                cadence_hint = "current grade, how often you train"
            elif domain == "product":
                cadence_hint = "what's left to ship, how many focused sessions you can do each week"
            elif domain == "reading":
                cadence_hint = "how much you read now, a realistic weekly cadence"
            else:
                cadence_hint = "where you're starting from and a realistic weekly cadence"
            reply = (
                f"Got it — I'll treat this as one goal: {title}. "
                f"Tell me {cadence_hint}. "
                "I'll shape a plan with you — nothing hits the calendar until you approve."
            )
            return PlanResult(reply=reply, goal_patch=patch)

    # Agreement to amount → ready
    if goal and re.search(r"\b(that works|sounds good|£\d+|go with|let'?s do)\b", lower):
        amt = _money_amounts(text)
        if amt:
            patch["commitment"] = f"£{amt[0]} once a month on payday"
            patch["frequency"] = "once a month"
            facts["suggested_monthly_save"] = amt[0]
        patch["status"] = "ready_to_plan"
        return PlanResult(
            reply=(
                "Great — I'll use that as the working commitment. "
                "Say the word and I'll check real calendar availability and propose dated payday sessions for you to approve."
            ),
            goal_patch=patch,
            ready_to_propose=True,
            pending_question="Shall I propose sessions on the calendar?",
        )

    # Generic helpful continue — never repeat a naked frequency wall
    title = (goal.title if goal else None) or patch.get("title") or "this goal"
    domain = patch.get("domain") or (goal.domain if goal else None)
    if domain == "savings":
        hints = "constraints, leftover money, preferred days"
    elif domain == "climbing":
        hints = "constraints, preferred training days, how often you can climb"
    elif domain == "product":
        hints = "constraints, what's left to ship, preferred work days"
    elif domain == "reading":
        hints = "constraints, preferred reading times, weekly cadence"
    else:
        hints = "constraints, preferred days, realistic cadence"
    return PlanResult(
        reply=(
            f"I'm still with you on {title}. Share whatever helps ({hints}), "
            "ask me to estimate, or tell me to propose calendar sessions when you're ready. "
            "I won't book anything without your approval."
        ),
        goal_patch=patch,
    )


def plan_turn(goal: Goal | None, message: str, recent: list[dict[str, str]] | None = None) -> PlanResult:
    recent = recent or []
    # Prefer model; heuristic always available as safety net / offline.
    try:
        modeled = plan_with_model(goal, message, recent)
        if modeled and modeled.reply:
            # Merge heuristic facts extraction lightly when model omits them
            heur = plan_heuristic(goal, message)
            if heur.goal_patch.get("facts"):
                facts = dict(heur.goal_patch["facts"])
                facts.update(modeled.goal_patch.get("facts") or {})
                modeled.goal_patch["facts"] = facts
            for key in ("deadline", "baseline", "target", "frequency", "commitment", "domain", "title"):
                if key not in modeled.goal_patch and key in heur.goal_patch:
                    modeled.goal_patch[key] = heur.goal_patch[key]
            return modeled
    except Exception:
        pass
    return plan_heuristic(goal, message)


def apply_goal_patch(goal: Goal, patch: dict[str, Any]) -> Goal:
    allowed = {
        "title",
        "domain",
        "target",
        "deadline",
        "baseline",
        "frequency",
        "commitment",
        "status",
    }
    for key in allowed:
        if key in patch and patch[key] is not None:
            setattr(goal, key, patch[key])
    if "facts" in patch and isinstance(patch["facts"], dict):
        merged = dict(goal.facts or {})
        merged.update(patch["facts"])
        goal.facts = merged
    # Promote to ready when we have a workable commitment
    if goal.commitment and goal.status == "gathering":
        goal.status = "ready_to_plan"
    if goal.status not in {"gathering", "ready_to_plan", "planned", "active", "paused", "done"}:
        goal.status = "gathering"
    return goal
