"""Fast-path vs MLX gating for classify→fill."""

from plan_pipeline import run_classify_fill
from prompts import CHAT_SYSTEM_PROMPT, respond_system_prompt


def _boom(_system: str, _messages: list) -> str:
    raise AssertionError("mlx_complete should not run")


def test_fast_path_skips_mlx():
    plan = run_classify_fill(
        message="What's on today?",
        history=[],
        memory_messages=[{"role": "user", "content": "What's on today?"}],
        available_tools="- calendar.look",
        mlx_complete=_boom,
    )
    assert plan.intent == "tool_use"
    assert plan.tool == "calendar.look"
    assert plan.next == "tool"


def test_heuristic_only_miss_skips_mlx():
    plan = run_classify_fill(
        message="how are you today?",
        history=[],
        memory_messages=[{"role": "user", "content": "how are you today?"}],
        available_tools="- calendar.look",
        mlx_complete=_boom,
        allow_mlx=False,
    )
    assert plan.intent == "chat"
    assert not plan.tool
    assert "heuristic_only" in (plan.reasoning or "")


def test_schedule_week_fast_path():
    plan = run_classify_fill(
        message="hey i want to book some time this week for climbing",
        history=[],
        memory_messages=[
            {
                "role": "user",
                "content": "hey i want to book some time this week for climbing",
            }
        ],
        available_tools="- calendar.organize",
        mlx_complete=_boom,
    )
    assert plan.intent == "tool_use"
    assert plan.tool == "calendar.organize"


def test_chat_respond_prompt_is_not_tool_narration():
    chat = respond_system_prompt(has_tool_result=False)
    tool = respond_system_prompt(has_tool_result=True)
    assert chat == CHAT_SYSTEM_PROMPT
    assert "no tools are attached" in chat
    assert "tool result" in tool.lower()


if __name__ == "__main__":
    test_fast_path_skips_mlx()
    test_heuristic_only_miss_skips_mlx()
    test_schedule_week_fast_path()
    test_chat_respond_prompt_is_not_tool_narration()
    print("ok plan pipeline")
