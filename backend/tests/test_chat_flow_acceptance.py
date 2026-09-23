"""Acceptance tests for the finished multi-goal chat control plane."""

from __future__ import annotations

from pathlib import Path

import pytest

from app.control_plane import ControlPlane, RevertBlocked
from tests.fake_ai import FakeGroq
from tests.test_calendar_actions import _book_sessions
from tests.test_control_plane_tightening import ScriptedAI


def _weekly(title: str, weekday: int = 1) -> dict:
    return {
        "weekly_plan": {
            "repeat": "weekly",
            "pattern_summary": f"{title} weekly",
            "prefer_after_hour": 17,
            "slots": [
                {
                    "weekday": weekday,
                    "title": title,
                    "start_hour": 18,
                    "start_minute": 0,
                    "duration_minutes": 60,
                }
            ],
        }
    }


def _dump_payload() -> dict:
    return {
        "schema_version": 2,
        "assistant_text": "Three goals, each with a plan to approve.",
        "intents": ["goal_create", "goal_plan_request"],
        "operations": [
            {
                "kind": "goal_create",
                "target_ref": "climb",
                "payload": {
                    "title": "Climb V6",
                    "domain": "climbing",
                    "status": "ready_to_plan",
                    "frequency": "twice a week",
                    "facts": _weekly("Climb", 1),
                },
                "confidence": 0.9,
            },
            {
                "kind": "propose_sessions",
                "target_ref": "climb",
                "payload": {},
                "confidence": 0.9,
            },
            {
                "kind": "goal_create",
                "target_ref": "read",
                "payload": {
                    "title": "Read two books",
                    "domain": "reading",
                    "status": "ready_to_plan",
                    "frequency": "once a week",
                    "facts": _weekly("Read", 3),
                },
                "confidence": 0.9,
            },
            {
                "kind": "propose_sessions",
                "target_ref": "read",
                "payload": {},
                "confidence": 0.9,
            },
            {
                "kind": "goal_create",
                "target_ref": "save",
                "payload": {
                    "title": "Save £3000",
                    "domain": "savings",
                    "status": "ready_to_plan",
                    "frequency": "once a week",
                    "facts": _weekly("Save", 5),
                },
                "confidence": 0.9,
            },
            {
                "kind": "propose_sessions",
                "target_ref": "save",
                "payload": {},
                "confidence": 0.9,
            },
        ],
        "goal_updates": [],
        "requested_action": {"type": "none"},
        "confidence": 0.9,
    }


def test_three_goals_keep_three_proposal_groups_after_reload(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=ScriptedAI(_dump_payload()))
    try:
        res = plane.handle_message(
            "I want to climb V6, read two books, and save £3,000 by December. Fit the work around my calendar."
        )
        groups = res.proposal_groups
        assert len(groups) == 3
        titles = {g.goal.title for g in groups if g.goal}
        assert titles == {"Climb V6", "Read two books", "Save £3000"}
        batch_ids = {g.proposal_batch_id for g in groups}
        assert len(batch_ids) == 3
        for group in groups:
            assert group.sessions
            assert all(s.goal_id == group.goal.id for s in group.sessions)
        reloaded = plane.get_open_proposal(res.conversation_id)
        assert len(reloaded["proposal_groups"]) == 3
        assert {g["proposal_batch_id"] for g in reloaded["proposal_groups"]} == batch_ids
    finally:
        plane.close()


def test_approving_one_proposal_group_leaves_the_others(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=ScriptedAI(_dump_payload()))
    try:
        res = plane.handle_message("Climb, read, and save — plan them.")
        groups = res.proposal_groups
        assert len(groups) == 3
        first = groups[0]
        decided = plane.decide_proposal(
            first.proposal_batch_id, "approve", conversation_id=res.conversation_id
        )
        assert decided.get("booked")
        remaining = plane.get_open_proposal(res.conversation_id)["proposal_groups"]
        assert len(remaining) == 2
        remaining_ids = {g["proposal_batch_id"] for g in remaining}
        assert first.proposal_batch_id not in remaining_ids
        for group in remaining:
            assert group["sessions"]
            assert group["goal"]["id"] != first.goal.id
    finally:
        plane.close()


def test_mutation_preview_exposes_before_and_after(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=FakeGroq())
    try:
        ready, booked = _book_sessions(plane)
        target = booked.booked_sessions[0]
        plane._ai = ScriptedAI(
            {
                "assistant_text": "I can remove that session if you approve.",
                "intents": ["calendar_delete"],
                "operations": [
                    {
                        "kind": "calendar_delete",
                        "target_type": "session",
                        "target_id": target.id,
                        "payload": {
                            "op": "delete",
                            "session_id": target.id,
                        },
                        "confidence": 0.9,
                    }
                ],
                "calendar_actions": [{"op": "delete", "session_id": target.id}],
                "requested_action": {"type": "none"},
                "confidence": 0.9,
            }
        )
        result = plane.handle_message("Delete that session", conversation_id=ready.conversation_id)
        assert plane.calendar.get_session(target.id) is not None
        preview = result.mutation_preview
        assert preview
        records = preview.get("records") or []
        assert len(records) == 1
        rec = records[0]
        assert rec["id"] == target.id
        assert rec["before"]["title"] == target.title
        assert rec["before"]["start_at"] == target.start_at
        assert rec["after"] is None
        assert rec.get("match_reason")
        assert result.operations
        assert any(op.get("status") == "needs_approval" for op in result.operations)
    finally:
        plane.close()


def test_stale_preview_does_not_mutate_unreviewed_records(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=FakeGroq())
    try:
        ready, booked = _book_sessions(plane)
        target = booked.booked_sessions[0]
        plane._ai = ScriptedAI(
            {
                "assistant_text": "I can remove matching sessions if you approve.",
                "intents": ["calendar_delete"],
                "operations": [
                    {
                        "kind": "calendar_delete",
                        "payload": {
                            "op": "delete",
                            "title_contains": target.title.split()[0],
                            "all_matching": True,
                        },
                        "confidence": 0.8,
                    }
                ],
                "calendar_actions": [
                    {
                        "op": "delete",
                        "title_contains": target.title.split()[0],
                        "all_matching": True,
                    }
                ],
                "requested_action": {"type": "none"},
            }
        )
        previewed = plane.handle_message("Delete those climbing sessions", conversation_id=ready.conversation_id)
        reviewed_ids = {r["id"] for r in (previewed.mutation_preview or {}).get("records") or []}
        assert target.id in reviewed_ids
        extra = plane.calendar.create_manual_session(
            title=target.title,
            start_at="2026-12-20T18:00:00+00:00",
            end_at="2026-12-20T19:00:00+00:00",
        )
        plane.calendar.update_session(target.id, title=target.title + " changed")
        approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
        assert extra.id not in (approved.deleted_session_ids or [])
        assert plane.calendar.get_session(extra.id) is not None
        assert approved.mutation_preview or "stale" in (approved.reply or "").lower()
        assert plane.calendar.get_session(target.id) is not None
    finally:
        plane.close()


def test_stale_second_operation_does_not_apply_the_first(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=FakeGroq())
    try:
        ready, booked = _book_sessions(plane)
        assert len(booked.booked_sessions) >= 2
        first, second = booked.booked_sessions[0], booked.booked_sessions[1]
        plane._ai = ScriptedAI(
            {
                "assistant_text": "I can remove those two sessions if you approve.",
                "intents": ["calendar_delete"],
                "operations": [
                    {
                        "kind": "calendar_delete",
                        "payload": {"op": "delete", "session_id": first.id},
                        "confidence": 0.9,
                    },
                    {
                        "kind": "calendar_delete",
                        "payload": {"op": "delete", "session_id": second.id},
                        "confidence": 0.9,
                    },
                ],
                "requested_action": {"type": "none"},
            }
        )
        previewed = plane.handle_message(
            "Delete those two sessions", conversation_id=ready.conversation_id
        )
        records = (previewed.mutation_preview or {}).get("records") or []
        assert {first.id, second.id} <= {r["id"] for r in records}
        plane.calendar.update_session(second.id, title=second.title + " changed")
        approved = plane.handle_message("Approve", conversation_id=ready.conversation_id)
        assert plane.calendar.get_session(first.id) is not None
        assert plane.calendar.get_session(second.id) is not None
        assert not (approved.deleted_session_ids or [])
        assert "stale" in (approved.reply or "").lower() or "didn't apply" in (approved.reply or "").lower()
        assert "applied" not in (approved.reply or "").lower()
        details = " ".join(
            f"{step.get('label') or ''} {step.get('detail') or ''}" for step in (approved.activity or [])
        ).lower()
        assert "no calendar" in details
    finally:
        plane.close()


def test_follow_up_updates_the_named_goal_not_the_latest(tmp_path: Path):
    dump = ScriptedAI(_dump_payload())
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=dump)
    try:
        created = plane.handle_message("Climb, read, and save.")
        climb = next(g.goal for g in created.proposal_groups if g.goal and "climb" in g.goal.title.lower())
        latest = created.proposal_groups[-1].goal
        assert latest is not None
        assert latest.id != climb.id
        plane._ai = ScriptedAI(
            {
                "assistant_text": "Climbing is now twice weekly.",
                "intents": ["goal_update"],
                "operations": [
                    {
                        "kind": "goal_update",
                        "payload": {"frequency": "twice a week", "status": "ready_to_plan"},
                        "confidence": 0.85,
                    }
                ],
                "goal_updates": [{"action": "update", "frequency": "twice a week", "status": "ready_to_plan"}],
                "requested_action": {"type": "none"},
            }
        )
        updated = plane.handle_message(
            "Make climbing twice weekly", conversation_id=created.conversation_id
        )
        climb_now = plane.goals.get(climb.id)
        latest_now = plane.goals.get(latest.id)
        assert climb_now.frequency == "twice a week"
        assert latest_now.frequency != "twice a week" or latest_now.id == climb.id
        assert updated.goal is None or updated.goal.id == climb.id or updated.operations
        op_goals = {op.get("goal_id") for op in updated.operations if op.get("kind") == "goal_update"}
        assert climb.id in op_goals
        assert latest.id not in op_goals
    finally:
        plane.close()


def test_partial_failure_is_not_reported_as_success(tmp_path: Path):
    plane = ControlPlane(
        db_path=tmp_path / "t.db",
        ai=ScriptedAI(
            {
                "assistant_text": "All done — everything succeeded.",
                "intents": ["goal_create", "calendar_delete"],
                "operations": [
                    {
                        "kind": "goal_create",
                        "payload": {"title": "Climb V6", "domain": "climbing", "status": "gathering"},
                    },
                    {
                        "kind": "calendar_delete",
                        "payload": {"op": "delete", "session_id": "missing-session"},
                    },
                ],
                "requested_action": {"type": "none"},
            }
        ),
    )
    try:
        res = plane.handle_message("Create a climb goal and delete a missing session")
        statuses = {op.get("status") for op in res.operations}
        assert "succeeded" in statuses
        assert statuses & {"failed", "needs_clarification", "needs_approval"}
        lower = res.reply.lower()
        assert "everything succeeded" not in lower
        assert "all done" not in lower
        assert any(word in lower for word in ("partial", "couldn't", "could not", "not apply", "failed"))
        assert plane.goals.list_open()
    finally:
        plane.close()


def test_clarification_answers_survive_reload(tmp_path: Path):
    plane = ControlPlane(
        db_path=tmp_path / "t.db",
        ai=ScriptedAI(
            {
                "assistant_text": "What grade do you climb now?",
                "intents": ["goal_create"],
                "operations": [
                    {
                        "kind": "goal_create",
                        "payload": {"title": "Climb V6", "domain": "climbing", "status": "gathering"},
                    }
                ],
                "clarification_questions": [
                    {
                        "id": "grade",
                        "label": "What grade do you climb consistently?",
                        "suggested_answer": "V4",
                        "answer_type": "short_text",
                    }
                ],
                "requested_action": {"type": "none"},
            }
        ),
    )
    try:
        res = plane.handle_message("I want to climb V6")
        assert res.clarification_questions
        plane.save_draft(
            res.conversation_id,
            {
                "clarification_questions": [q.model_dump() for q in res.clarification_questions],
                "clarification_answers": {"grade": "V3 indoors"},
            },
        )
        other = plane.create_conversation()
        restored = plane.get_open_proposal(res.conversation_id)
        questions = restored.get("clarification_questions") or []
        answers = restored.get("entered_answers") or restored.get("clarification_answers") or {}
        assert questions
        assert answers.get("grade") == "V3 indoors"
        suggested = questions[0].get("suggested_answer") if isinstance(questions[0], dict) else questions[0].suggested_answer
        assert suggested == "V4"
        assert answers.get("grade") != suggested
        other_open = plane.get_open_proposal(other["id"])
        assert not (other_open.get("clarification_questions") or [])
        back = plane.get_open_proposal(res.conversation_id)
        assert (back.get("entered_answers") or back.get("clarification_answers") or {}).get("grade") == "V3 indoors"
    finally:
        plane.close()


def _chat_payload() -> dict:
    return {
        "schema_version": 2,
        "assistant_text": "I'm with you.",
        "intents": ["chat"],
        "operations": [],
        "goal_updates": [],
        "requested_action": {"type": "none"},
        "confidence": 0.5,
    }


def test_edit_undoes_later_goal_create(tmp_path: Path):
    ai = ScriptedAI(_chat_payload())
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=ai)
    try:
        first = plane.handle_message("Just thinking")
        ai.payload = {
            "schema_version": 2,
            "assistant_text": "Climbing is on the list.",
            "intents": ["goal_create"],
            "operations": [
                {
                    "kind": "goal_create",
                    "payload": {"title": "Climb V6", "domain": "climbing", "status": "gathering"},
                    "confidence": 0.9,
                }
            ],
            "confidence": 0.9,
        }
        plane.handle_message("I want to climb V6", conversation_id=first.conversation_id)
        titles = [g.title.lower() for g in plane.list_goals()]
        assert any("climb" in t for t in titles)
        first_user = next(
            m for m in plane.list_messages(first.conversation_id) if m["role"] == "user"
        )
        ai.payload = _chat_payload()
        plane.handle_message(
            "Just thinking out loud",
            conversation_id=first.conversation_id,
            revision_of=first_user["id"],
        )
        titles = [g.title.lower() for g in plane.list_goals()]
        assert not any("climb" in t for t in titles)
        visible = plane.list_messages(first.conversation_id)
        user_lines = [m["content"] for m in visible if m["role"] == "user"]
        assert user_lines == ["Just thinking out loud"]
    finally:
        plane.close()


def test_edit_undoes_spark_capture(tmp_path: Path):
    ai = ScriptedAI(_chat_payload())
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=ai)
    try:
        first = plane.handle_message("Just thinking")
        ai.payload = {
            "schema_version": 2,
            "assistant_text": "Saved spark (not a commitment): standing desk",
            "intents": ["spark_capture"],
            "operations": [
                {
                    "kind": "spark_capture",
                    "payload": {"spark_content": "standing desk"},
                    "confidence": 0.9,
                }
            ],
            "requested_action": {"type": "none", "spark_content": "standing desk"},
            "confidence": 0.9,
        }
        plane.handle_message("spark: standing desk", conversation_id=first.conversation_id)
        assert any("standing desk" in s.content.lower() for s in plane.sparks.list_open())
        spark_user = [
            m
            for m in plane.list_messages(first.conversation_id)
            if m["role"] == "user" and "standing desk" in m["content"].lower()
        ][-1]
        ai.payload = _chat_payload()
        plane.handle_message(
            "never mind the spark",
            conversation_id=first.conversation_id,
            revision_of=spark_user["id"],
        )
        assert not any("standing desk" in s.content.lower() for s in plane.sparks.list_all())
        assert not plane.sparks.list_open()
    finally:
        plane.close()


def test_revert_blocked_when_completed_session_exists(tmp_path: Path):
    plane = ControlPlane(db_path=tmp_path / "t.db", ai=FakeGroq())
    try:
        ready, booked = _book_sessions(plane)
        sid = booked.booked_sessions[0].id
        plane.mark_outcome(sid, "completed")
        first_user = next(
            m for m in plane.list_messages(ready.conversation_id) if m["role"] == "user"
        )
        result = plane.revert_to(ready.conversation_id, first_user["id"])
        assert result["ok"] is False
        assert result.get("blocked") is True
        session = plane.calendar.get_session(sid)
        assert session is not None
        assert session.status == "completed"
        with pytest.raises(RevertBlocked):
            plane.handle_message(
                "rewrite the start",
                conversation_id=ready.conversation_id,
                revision_of=first_user["id"],
            )
        still = plane.calendar.get_session(sid)
        assert still is not None and still.status == "completed"
    finally:
        plane.close()
