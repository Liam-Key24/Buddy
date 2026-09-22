"""Failing cases for chat control-plane tightening.

These capture current bugs: pause_others on multi-create, immediate destructive
calendar writes, non-idempotent request_id, Groq 4xx retries, duplicated user
message in context, and always-on calendar activity.
"""

from __future__ import annotations

import json
from pathlib import Path

import httpx
import pytest

from app.ai.groq_provider import GroqError, GroqProvider
from app.config import DEFAULT_CORS_ORIGINS, Settings
from app.control_plane import ControlPlane
from tests.fake_ai import FakeGroq
from tests.test_calendar_actions import _book_sessions


class ScriptedAI:
    def __init__(self, payload: dict):
        self.payload = payload
        self.calls = 0
        self.last_user: dict | None = None

    def complete_json(self, system: str, user: str, *, allow_retry: bool = True, **kwargs) -> dict:
        self.calls += 1
        self.last_user = json.loads(user)
        return self.payload


@pytest.fixture()
def plane(tmp_path: Path):
    ai = FakeGroq()
    svc = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    yield svc
    svc.close()


def _settings(**kwargs) -> Settings:
    values = dict(
        db_path=Path("."),
        host="127.0.0.1",
        port=8787,
        groq_api_key="test-key",
        groq_model="openai/gpt-oss-120b",
        groq_base_url="https://api.groq.com/openai/v1",
        ai_enabled=True,
        request_timeout_s=5.0,
        max_output_tokens=128,
        cors_origins=DEFAULT_CORS_ORIGINS,
    )
    values.update(kwargs)
    return Settings(**values)


def test_multi_goal_dump_keeps_all_goals_open(tmp_path: Path):
    ai = ScriptedAI(
        {
            "assistant_text": "Captured climb, reading, savings, and a spark.",
            "intents": ["goal_create", "spark_capture"],
            "goal_updates": [
                {"action": "create", "title": "Climb V6", "domain": "climbing", "status": "gathering"},
                {"action": "create", "title": "Read 12 books", "domain": "reading", "status": "gathering"},
                {"action": "create", "title": "Save £4000", "domain": "savings", "status": "gathering"},
            ],
            "requested_action": {"type": "none", "spark_content": "try a standing desk"},
            "confidence": 0.9,
        }
    )
    plane = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    try:
        res = plane.handle_message(
            "I want to climb V6, read 12 books, save £4000, and spark: try a standing desk"
        )
        open_goals = plane.goals.list_open()
        titles = {g.title for g in open_goals}
        assert titles == {"Climb V6", "Read 12 books", "Save £4000"}
        assert all(g.status != "paused" for g in open_goals)
        assert any("standing desk" in s.content.lower() for s in plane.sparks.list_open())
        assert hasattr(res, "operations")
        assert len(res.operations) >= 4
    finally:
        plane.close()


def test_chat_delete_previews_instead_of_committing(plane: ControlPlane):
    ready, booked = _book_sessions(plane)
    target_id = booked.booked_sessions[0].id
    result = plane.handle_message("Delete that session", conversation_id=ready.conversation_id)
    still = plane.calendar.get_session(target_id)
    assert still is not None
    assert target_id not in (result.deleted_session_ids or [])
    assert result.operations
    preview = [op for op in result.operations if op.get("status") == "needs_approval"]
    assert preview, "destructive calendar ops must wait for approval"


def test_duplicate_request_id_replays_without_double_write(tmp_path: Path):
    ai = ScriptedAI(
        {
            "assistant_text": "Saved spark (not a commitment): standing desk",
            "intents": ["spark_capture"],
            "goal_updates": [],
            "requested_action": {"type": "none", "spark_content": "standing desk"},
            "confidence": 0.9,
        }
    )
    plane = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    try:
        rid = "req-same-turn"
        first = plane.handle_message("spark: standing desk", request_id=rid)
        second = plane.handle_message(
            "spark: standing desk",
            conversation_id=first.conversation_id,
            request_id=rid,
        )
        assert first.reply == second.reply
        assert first.conversation_id == second.conversation_id
        assert ai.calls == 1
        sparks = plane.sparks.list_open()
        assert len(sparks) == 1
        messages = plane.list_messages(first.conversation_id)
        user_lines = [m for m in messages if m["role"] == "user"]
        assert len(user_lines) == 1
    finally:
        plane.close()


def test_cancel_closes_only_the_request_client():
    shared = httpx.Client()
    provider = GroqProvider(_settings(), client=shared)
    plane = ControlPlane(ai=provider)
    plane._ai_owned = False
    dedicated = httpx.Client()
    try:
        plane._active_requests["r1"] = {
            "cancelled": False,
            "http_client": dedicated,
            "owns_http_client": True,
        }
        plane.cancel_request("r1")
        assert dedicated.is_closed
        assert not shared.is_closed
        plane._active_requests["r2"] = {
            "cancelled": False,
            "http_client": shared,
            "owns_http_client": False,
        }
        plane.cancel_request("r2")
        assert not shared.is_closed
    finally:
        shared.close()
        plane.close()


def test_groq_does_not_retry_ordinary_4xx():
    posts = {"n": 0}

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path.endswith("/models"):
            return httpx.Response(200, json={"data": [{"id": "openai/gpt-oss-120b"}]})
        posts["n"] += 1
        return httpx.Response(400, json={"error": {"message": "bad request"}})

    client = httpx.Client(
        transport=httpx.MockTransport(handler),
        base_url="https://api.groq.com/openai/v1",
    )
    provider = GroqProvider(_settings(), client=client)
    with pytest.raises(GroqError) as exc:
        provider.complete_json("system", "user")
    assert exc.value.category == "http"
    assert posts["n"] == 1


def test_json_validate_failed_is_malformed_and_not_retried_by_provider():
    posts = {"n": 0}

    def handler(request: httpx.Request) -> httpx.Response:
        if request.url.path.endswith("/models"):
            return httpx.Response(200, json={"data": [{"id": "openai/gpt-oss-120b"}]})
        posts["n"] += 1
        return httpx.Response(
            400,
            json={"error": {"code": "json_validate_failed", "message": "failed to validate json"}},
        )

    client = httpx.Client(
        transport=httpx.MockTransport(handler),
        base_url="https://api.groq.com/openai/v1",
    )
    provider = GroqProvider(_settings(), client=client)
    with pytest.raises(GroqError) as exc:
        provider.complete_json("system", "user")
    assert exc.value.category == "malformed"
    assert posts["n"] == 1


def test_parse_json_object_skips_preamble():
    from app.ai.groq_provider import _parse_json_object

    data = _parse_json_object(
        'note {"foo": 1}\n{"assistant_text": "Hi", "intents": ["chat"]}\nThanks'
    )
    assert data["assistant_text"] == "Hi"
    assert data["intents"] == ["chat"]


def test_json_validate_failed_gets_one_control_plane_repair(tmp_path: Path):
    class RepairThenOk:
        def __init__(self):
            self.n = 0

        def complete_json(self, system: str, user: str, *, allow_retry: bool = True, **kwargs) -> dict:
            del system, user, allow_retry, kwargs
            self.n += 1
            if self.n == 1:
                raise GroqError("malformed", "Cloud AI returned unusable JSON")
            return {
                "assistant_text": "I'm with you.",
                "intents": ["chat"],
                "goal_updates": [],
                "requested_action": {"type": "none"},
                "confidence": 0.5,
            }

        def close(self) -> None:
            return None

    ai = RepairThenOk()
    plane = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    try:
        res = plane.handle_message("Just a chat")
        assert ai.n == 2
        assert "with you" in res.reply.lower()
    finally:
        plane.close()


def test_user_message_is_not_duplicated_in_history(tmp_path: Path):
    ai = ScriptedAI(
        {
            "assistant_text": "I'm with you.",
            "intents": ["chat"],
            "goal_updates": [],
            "requested_action": {"type": "none"},
            "confidence": 0.5,
        }
    )
    plane = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    try:
        plane.handle_message("Keep this first line")
        plane.handle_message("second turn please")
        payload = ai.last_user
        assert payload is not None
        assert payload["user_message"] == "second turn please"
        history_user = [
            m["content"] for m in payload["recent_messages"] if m.get("role") == "user"
        ]
        assert "second turn please" not in history_user
        assert payload.get("timezone") == "Europe/London"
        # Chat-only turns must not send the whole calendar.
        assert payload.get("calendar_sessions") in ([], None)
    finally:
        plane.close()


def test_london_bst_tomorrow(tmp_path: Path):
    from datetime import datetime
    from zoneinfo import ZoneInfo

    from app.timeutil import resolve_relative_date

    # 22 June 2026 23:30 UTC is already 23 June 00:30 in London BST.
    at = datetime(2026, 6, 22, 23, 30, tzinfo=ZoneInfo("UTC"))
    assert resolve_relative_date("tomorrow", "Europe/London", at=at) == "2026-06-24"
    assert resolve_relative_date("today", "Europe/London", at=at) == "2026-06-23"


def test_revert_hides_later_messages(tmp_path: Path):
    ai = ScriptedAI(
        {
            "assistant_text": "I'm with you.",
            "intents": ["chat"],
            "goal_updates": [],
            "requested_action": {"type": "none"},
            "confidence": 0.5,
        }
    )
    plane = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    try:
        first = plane.handle_message("Keep this first line")
        plane.handle_message("second turn please", conversation_id=first.conversation_id)
        messages = plane.list_messages(first.conversation_id)
        first_user = next(m for m in messages if m["role"] == "user")
        plane.revert_to(first.conversation_id, first_user["id"])
        visible = plane.list_messages(first.conversation_id)
        user_lines = [m["content"] for m in visible if m["role"] == "user"]
        assert user_lines == ["Keep this first line"]
        again = plane.revert_to(first.conversation_id, first_user["id"])
        assert again["ok"] is True
    finally:
        plane.close()


def test_unknown_requested_action_does_not_succeed(tmp_path: Path):
    ai = ScriptedAI(
        {
            "assistant_text": "I wiped the calendar.",
            "intents": ["chat"],
            "goal_updates": [],
            "requested_action": {"type": "explode_calendar"},
            "confidence": 0.9,
        }
    )
    plane = ControlPlane(db_path=tmp_path / "test.db", ai=ai)
    try:
        res = plane.handle_message("Please explode the calendar")
        assert res.unresolved
        assert any(op.get("status") == "failed" for op in res.operations)
        assert "wiped" not in res.reply.lower() or res.unresolved
    finally:
        plane.close()
