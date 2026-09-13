from complete import (
    ThinkStreamFilter,
    apply_no_think,
    parse_completion_message,
    parse_qwen_xml_tool_calls,
    strip_think,
)


class _Fn:
    def __init__(self, name, arguments):
        self.name = name
        self.arguments = arguments


class _Tc:
    def __init__(self, name, arguments, call_id="call_0"):
        self.id = call_id
        self.function = _Fn(name, arguments)


class _Msg:
    def __init__(self, content=None, tool_calls=None):
        self.content = content
        self.tool_calls = tool_calls or []


def test_strip_think_block():
    assert strip_think("<think>secret</think>Hello") == "Hello"
    assert "secret" not in strip_think("<think>unclosed")


def test_qwen_json_tool_call():
    content = '<tool_call>{"name":"calendar.look","arguments":{"when":"today"}}</tool_call>'
    calls = parse_qwen_xml_tool_calls(content)
    assert len(calls) == 1
    assert calls[0]["name"] == "calendar.look"
    assert calls[0]["arguments"]["when"] == "today"


def test_openai_tool_calls_win():
    msg = _Msg(
        content="<think>x</think>",
        tool_calls=[_Tc("calendar.pin", '{"title":"Dentist","start":"tomorrow 14:00"}')],
    )
    text, calls = parse_completion_message(msg, msg.content)
    assert text == ""
    assert calls[0]["name"] == "calendar.pin"
    assert calls[0]["arguments"]["title"] == "Dentist"


def test_apply_no_think_last_user():
    msgs = apply_no_think(
        [
            {"role": "system", "content": "sys"},
            {"role": "user", "content": "whats the capital of the uk"},
        ]
    )
    assert msgs[-1]["content"].endswith("/no_think")
    again = apply_no_think(msgs)
    assert again[-1]["content"].count("/no_think") == 1


def test_cool_mode_token_caps():
    import inspect

    from mlx_client import AGENT_MAX_TOKENS, CHAT_MAX_TOKENS, MLXClient, NARRATE_MAX_TOKENS

    assert AGENT_MAX_TOKENS == 1024
    assert CHAT_MAX_TOKENS == 256
    assert NARRATE_MAX_TOKENS == 256
    src = inspect.getsource(MLXClient.__init__)
    assert "timeout=180" in src.replace(" ", "") or "timeout=180.0" in src


def test_think_stream_filter_drops_reasoning():
    filt = ThinkStreamFilter()
    assert filt.push("<think>secret") == ""
    assert filt.push(" still secret</th") == ""
    assert filt.push("ink>London.") == "London."


if __name__ == "__main__":
    test_strip_think_block()
    test_qwen_json_tool_call()
    test_openai_tool_calls_win()
    test_apply_no_think_last_user()
    test_cool_mode_token_caps()
    test_think_stream_filter_drops_reasoning()
    print("ok complete parser")
