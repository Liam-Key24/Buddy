"""Parse MLX / Qwen chat completions into OpenAI-style tool_calls."""

from __future__ import annotations

import json
import re
from typing import Any

THINK_RE = re.compile(r"<think>.*?</think>", re.DOTALL | re.IGNORECASE)
UNCLOSED_THINK_RE = re.compile(r"<think>.*", re.DOTALL | re.IGNORECASE)
TOOL_CALL_RE = re.compile(r"<tool_call>\s*(.*?)\s*</tool_call>", re.DOTALL | re.IGNORECASE)
ARG_KEY_RE = re.compile(r"<arg_key>\s*(.*?)\s*</arg_key>", re.DOTALL | re.IGNORECASE)
ARG_VAL_RE = re.compile(r"<arg_value>\s*(.*?)\s*</arg_value>", re.DOTALL | re.IGNORECASE)


def strip_think(text: str | None) -> str:
    if not text:
        return ""
    out = THINK_RE.sub("", text)
    out = UNCLOSED_THINK_RE.sub("", out)
    return out.strip()


NO_THINK_MARK = "/no_think"
_THINK_OPEN = "<think>"
_THINK_CLOSE = "</think>"


def apply_no_think(messages: list[dict]) -> list[dict]:
    """Qwen3: append /no_think to the last user turn so the model skips reasoning."""
    out = [dict(m) for m in messages]
    for i in range(len(out) - 1, -1, -1):
        if out[i].get("role") != "user":
            continue
        content = out[i].get("content") or ""
        if NO_THINK_MARK not in content:
            out[i]["content"] = f"{content.rstrip()}\n{NO_THINK_MARK}"
        break
    return out


def _find_ci(hay: str, needle: str) -> int:
    return hay.lower().find(needle.lower())


def _open_tag_prefix_len(buf: str) -> int:
    lower = buf.lower()
    for n in range(1, len(_THINK_OPEN)):
        if lower.endswith(_THINK_OPEN[:n]):
            return n
    return 0


class ThinkStreamFilter:
    """Drop <think>…</think> across streamed deltas so hidden reasoning never reaches the UI."""

    def __init__(self) -> None:
        self.in_think = False
        self.buf = ""

    def push(self, delta: str) -> str:
        if not delta:
            return ""
        self.buf += delta
        out: list[str] = []
        while self.buf:
            if self.in_think:
                end = _find_ci(self.buf, _THINK_CLOSE)
                if end < 0:
                    self.buf = self.buf[-len(_THINK_CLOSE) :]
                    break
                self.buf = self.buf[end + len(_THINK_CLOSE) :]
                self.in_think = False
                continue
            start = _find_ci(self.buf, _THINK_OPEN)
            if start < 0:
                keep = _open_tag_prefix_len(self.buf)
                if keep:
                    out.append(self.buf[:-keep])
                    self.buf = self.buf[-keep:]
                else:
                    out.append(self.buf)
                    self.buf = ""
                break
            out.append(self.buf[:start])
            self.buf = self.buf[start + len(_THINK_OPEN) :]
            self.in_think = True
        return "".join(out)


def _as_args(value: Any) -> dict[str, Any]:
    if value is None:
        return {}
    if isinstance(value, dict):
        return value
    if isinstance(value, str):
        trimmed = value.strip()
        if not trimmed:
            return {}
        try:
            parsed = json.loads(trimmed)
            return parsed if isinstance(parsed, dict) else {"value": parsed}
        except json.JSONDecodeError:
            return {"_raw": trimmed}
    return {"value": value}


def _call(name: str, arguments: Any, call_id: str | None = None) -> dict[str, Any]:
    return {
        "id": call_id or f"call_{name}",
        "name": name,
        "arguments": _as_args(arguments),
    }


def parse_qwen_xml_tool_calls(content: str) -> list[dict[str, Any]]:
    calls: list[dict[str, Any]] = []
    for idx, match in enumerate(TOOL_CALL_RE.finditer(content or "")):
        body = match.group(1).strip()
        if not body:
            continue
        try:
            data = json.loads(body)
        except json.JSONDecodeError:
            data = None
        if isinstance(data, dict) and data.get("name"):
            calls.append(
                _call(
                    str(data["name"]),
                    data.get("arguments", data.get("parameters", {})),
                    data.get("id") or f"call_{idx}",
                )
            )
            continue
        keys = ARG_KEY_RE.findall(body)
        vals = ARG_VAL_RE.findall(body)
        if keys:
            name_line = body.splitlines()[0].strip()
            name = name_line.split()[0] if name_line else ""
            args = {k.strip(): v.strip() for k, v in zip(keys, vals)}
            if name:
                calls.append(_call(name, args, f"call_{idx}"))
    return calls


def parse_openai_tool_calls(message: Any) -> list[dict[str, Any]]:
    raw = getattr(message, "tool_calls", None) or []
    calls: list[dict[str, Any]] = []
    for idx, tc in enumerate(raw):
        fn = getattr(tc, "function", None)
        name = getattr(fn, "name", None) if fn is not None else None
        if not name:
            continue
        arguments = getattr(fn, "arguments", None) if fn is not None else None
        calls.append(_call(str(name), arguments, getattr(tc, "id", None) or f"call_{idx}"))
    return calls


def parse_completion_message(message: Any, content: str | None) -> tuple[str, list[dict[str, Any]]]:
    text = strip_think(content if content is not None else getattr(message, "content", None))
    calls = parse_openai_tool_calls(message)
    if not calls:
        calls = parse_qwen_xml_tool_calls(text)
        if calls:
            text = TOOL_CALL_RE.sub("", text).strip()
    return text, calls
