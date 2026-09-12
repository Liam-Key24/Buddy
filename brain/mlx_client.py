from typing import Any, Iterable

from openai import OpenAI

from complete import ThinkStreamFilter, apply_no_think, parse_completion_message, strip_think

AGENT_MAX_TOKENS = 2048
CHAT_MAX_TOKENS = 256
NARRATE_MAX_TOKENS = 512
AGENT_TEMPERATURE = 0.2


class MLXClient:
    def __init__(self, base_url: str, model: str):
        self.client = OpenAI(base_url=base_url, api_key="not-needed", timeout=90.0)
        self.model = model

    def _create(self, **kwargs):
        kwargs.setdefault("extra_body", {})
        extra = dict(kwargs["extra_body"] or {})
        extra.setdefault("chat_template_kwargs", {})
        extra["chat_template_kwargs"] = {
            **extra.get("chat_template_kwargs", {}),
            "enable_thinking": False,
        }
        extra["enable_thinking"] = False
        kwargs["extra_body"] = extra
        return self.client.chat.completions.create(**kwargs)

    def complete(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int = AGENT_MAX_TOKENS,
        temperature: float = AGENT_TEMPERATURE,
    ) -> str:
        full_messages = apply_no_think([{"role": "system", "content": system}, *messages])
        response = self._create(
            model=self.model,
            messages=full_messages,
            max_tokens=max_tokens,
            temperature=temperature,
        )
        return strip_think(response.choices[0].message.content or "")

    def complete_with_tools(
        self,
        messages: list[dict],
        tools: list[dict] | None = None,
        max_tokens: int = AGENT_MAX_TOKENS,
        temperature: float = AGENT_TEMPERATURE,
    ) -> dict[str, Any]:
        """Forward messages+tools to MLX; return content + normalized tool_calls."""
        create_kwargs: dict[str, Any] = {
            "model": self.model,
            "messages": apply_no_think(messages),
            "max_tokens": max_tokens,
            "temperature": temperature,
        }
        used_tools = bool(tools)
        if tools:
            create_kwargs["tools"] = tools
        try:
            response = self._create(**create_kwargs)
        except TypeError:
            create_kwargs.pop("tools", None)
            used_tools = False
            response = self._create(**create_kwargs)
        except Exception:
            if "tools" not in create_kwargs:
                raise
            # Stock mlx_lm.server may reject tools= — prompt-inject and parse XML.
            create_kwargs.pop("tools", None)
            used_tools = False
            injected = _inject_tools_into_messages(messages, tools or [])
            create_kwargs["messages"] = apply_no_think(injected)
            response = self._create(**create_kwargs)

        choice = response.choices[0]
        message = choice.message
        content, tool_calls = parse_completion_message(message, message.content)
        if not tool_calls and not used_tools:
            content, tool_calls = parse_completion_message(message, content)
        return {
            "content": content,
            "tool_calls": tool_calls,
            "finish_reason": getattr(choice, "finish_reason", None) or (
                "tool_calls" if tool_calls else "stop"
            ),
        }

    def stream(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int = AGENT_MAX_TOKENS,
        temperature: float = 0.7,
    ) -> Iterable[str]:
        full_messages = apply_no_think([{"role": "system", "content": system}, *messages])
        stream = self._create(
            model=self.model,
            messages=full_messages,
            max_tokens=max_tokens,
            temperature=temperature,
            stream=True,
        )
        filt = ThinkStreamFilter()
        for chunk in stream:
            delta = chunk.choices[0].delta.content
            if not delta:
                continue
            visible = filt.push(delta)
            if visible:
                yield visible

    def stream_simple(
        self,
        system: str,
        messages: list[dict],
        max_tokens: int = CHAT_MAX_TOKENS,
        temperature: float = 0.7,
        model: str | None = None,
    ) -> Iterable[str]:
        """Plain Llama-style chat — no /no_think, no tool extras."""
        full_messages = [{"role": "system", "content": system}, *messages]
        stream = self.client.chat.completions.create(
            model=model or self.model,
            messages=full_messages,
            max_tokens=max_tokens,
            temperature=temperature,
            stream=True,
        )
        for chunk in stream:
            delta = chunk.choices[0].delta.content
            if delta:
                yield delta


def _inject_tools_into_messages(messages: list[dict], tools: list[dict]) -> list[dict]:
    names = []
    for tool in tools:
        fn = tool.get("function") or {}
        name = fn.get("name") or tool.get("name")
        if name:
            names.append(f"- {name}: {fn.get('description', '')}".rstrip())
    block = (
        "You may call tools by emitting XML (no markdown fences):\n"
        '<tool_call>{"name":"tool.name","arguments":{}}</tool_call>\n'
        "Available tools:\n"
        + ("\n".join(names) if names else "- (none)")
    )
    out = [dict(m) for m in messages]
    if out and out[0].get("role") == "system":
        out[0]["content"] = f"{out[0].get('content') or ''}\n\n{block}".strip()
    else:
        out.insert(0, {"role": "system", "content": block})
    return out
