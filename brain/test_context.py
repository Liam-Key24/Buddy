from context import MemoryContextPayload, build_messages


def test_chat_skips_memory_block():
    memory = MemoryContextPayload(handover="long project dump " * 20)
    msgs = build_messages(
        memory,
        [{"role": "user", "content": "hi"}, {"role": "assistant", "content": "hey"}],
        "whats the capital of the uk",
        include_memory=False,
        history_limit=8,
    )
    assert msgs[-1]["content"] == "whats the capital of the uk"
    assert "Project Handover" not in msgs[-1]["content"]


def test_tool_narration_keeps_memory():
    memory = MemoryContextPayload(handover="remember this")
    msgs = build_messages(
        memory,
        [],
        "ok",
        include_memory=True,
    )
    assert "remember this" in msgs[-1]["content"]


if __name__ == "__main__":
    test_chat_skips_memory_block()
    test_tool_narration_keeps_memory()
    print("ok context")
