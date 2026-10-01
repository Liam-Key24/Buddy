"""Each chat resends the system prompt, so it has to stay small."""

from __future__ import annotations

from app.ai.prompt import SYSTEM_PROMPT, build_user_payload


def test_system_prompt_is_small_enough_for_groq_tpm():
    # ~4 chars/token. The previous prompt was ~8.8k chars (~2.2k tokens) and
    # seven chats showed up as ~20k uncached input tokens.
    assert len(SYSTEM_PROMPT) < 4500


def test_user_payload_does_not_repeat_the_system_instructions():
    payload = build_user_payload(
        message="save £3000 by April",
        active_goal=None,
        recent_messages=[],
        open_proposal_batch_id=None,
        open_sparks=[],
    )
    assert "notes" not in payload
    assert "Do not ask which weekdays" not in payload
    assert len(payload) < 800
