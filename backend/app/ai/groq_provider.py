"""Groq Cloud AI provider — one HTTPS chat completion, no SDK required."""

from __future__ import annotations

import json
import logging
import time
from dataclasses import dataclass
from typing import Any

import httpx

from ..config import Settings

log = logging.getLogger("buddy.ai.groq")


class GroqError(Exception):
    def __init__(self, category: str, message: str):
        super().__init__(message)
        self.category = category


@dataclass
class GroqCallStats:
    latency_ms: int
    status: int | None
    tokens_prompt: int | None = None
    tokens_completion: int | None = None
    retried: bool = False
    error_category: str | None = None


class GroqProvider:
    """Thin Groq chat client. Never logs message content or API keys."""

    def __init__(self, settings: Settings, client: httpx.Client | None = None):
        self.settings = settings
        self._client = client
        self._owns_client = client is None
        self._model_validated = False
        self.last_stats: GroqCallStats | None = None

    def close(self) -> None:
        if self._owns_client and self._client is not None:
            self._client.close()
            self._client = None

    def _http(self) -> httpx.Client:
        if self._client is None:
            self._client = httpx.Client(
                base_url=self.settings.groq_base_url,
                timeout=self.settings.request_timeout_s,
                headers={
                    "Authorization": f"Bearer {self.settings.groq_api_key}",
                    "Content-Type": "application/json",
                },
            )
        return self._client

    def validate_model(self) -> None:
        """Confirm GROQ_MODEL exists. Never silently switches models."""
        if self._model_validated:
            return
        if not self.settings.groq_api_key:
            raise GroqError("config", "GROQ_API_KEY is not set")
        try:
            resp = self._http().get("/models")
        except httpx.HTTPError as exc:
            raise GroqError("transport", f"Could not reach Cloud AI: {exc.__class__.__name__}") from exc
        if resp.status_code == 401:
            raise GroqError("auth", "Cloud AI rejected the API key")
        if resp.status_code >= 400:
            raise GroqError("http", f"Cloud AI models check failed ({resp.status_code})")
        data = resp.json()
        ids = {item.get("id") for item in data.get("data", []) if isinstance(item, dict)}
        if self.settings.groq_model not in ids:
            raise GroqError(
                "config",
                f"Configured model {self.settings.groq_model!r} is not available on this Groq account. "
                "Set GROQ_MODEL to a model your account exposes (default openai/gpt-oss-120b).",
            )
        self._model_validated = True
        log.info("groq_model_ok model=%s", self.settings.groq_model)

    def complete_json(self, system: str, user: str, *, allow_retry: bool = True) -> dict[str, Any]:
        """One chat completion expecting a JSON object. At most one retry."""
        self.validate_model()
        payload = {
            "model": self.settings.groq_model,
            "temperature": 0.2,
            "max_tokens": self.settings.max_output_tokens,
            "response_format": {"type": "json_object"},
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        }
        try:
            return self._request_json(payload, retried=False)
        except GroqError as exc:
            if not allow_retry or exc.category not in {"transport", "malformed", "http"}:
                raise
            if exc.category == "http" and "429" not in str(exc) and "5" not in str(exc)[:20]:
                # Only retry rate-limit / 5xx-style categories flagged below
                pass
            return self._request_json(payload, retried=True)

    def _request_json(self, payload: dict[str, Any], *, retried: bool) -> dict[str, Any]:
        started = time.perf_counter()
        status: int | None = None
        try:
            resp = self._http().post("/chat/completions", json=payload)
            status = resp.status_code
            if status == 429:
                raise GroqError("rate_limit", "Cloud AI is rate-limited right now")
            if status >= 500:
                raise GroqError("http", f"Cloud AI server error ({status})")
            if status >= 400:
                raise GroqError("http", f"Cloud AI request failed ({status})")
            body = resp.json()
            content = body["choices"][0]["message"]["content"]
            usage = body.get("usage") or {}
            self.last_stats = GroqCallStats(
                latency_ms=int((time.perf_counter() - started) * 1000),
                status=status,
                tokens_prompt=usage.get("prompt_tokens"),
                tokens_completion=usage.get("completion_tokens"),
                retried=retried,
            )
            log.info(
                "groq_ok latency_ms=%s status=%s prompt_tokens=%s completion_tokens=%s retried=%s",
                self.last_stats.latency_ms,
                status,
                self.last_stats.tokens_prompt,
                self.last_stats.tokens_completion,
                retried,
            )
            return _parse_json_object(content)
        except GroqError as exc:
            self.last_stats = GroqCallStats(
                latency_ms=int((time.perf_counter() - started) * 1000),
                status=status,
                retried=retried,
                error_category=exc.category,
            )
            log.warning(
                "groq_err category=%s latency_ms=%s status=%s retried=%s",
                exc.category,
                self.last_stats.latency_ms,
                status,
                retried,
            )
            raise
        except (KeyError, IndexError, TypeError, json.JSONDecodeError) as exc:
            self.last_stats = GroqCallStats(
                latency_ms=int((time.perf_counter() - started) * 1000),
                status=status,
                retried=retried,
                error_category="malformed",
            )
            raise GroqError("malformed", "Cloud AI returned unusable output") from exc
        except httpx.HTTPError as exc:
            self.last_stats = GroqCallStats(
                latency_ms=int((time.perf_counter() - started) * 1000),
                status=status,
                retried=retried,
                error_category="transport",
            )
            raise GroqError("transport", f"Cloud AI unreachable ({exc.__class__.__name__})") from exc


def _parse_json_object(text: str) -> dict[str, Any]:
    text = (text or "").strip()
    if text.startswith("```"):
        text = text.removeprefix("```json").removeprefix("```").removesuffix("```").strip()
    data = json.loads(text)
    if not isinstance(data, dict):
        raise json.JSONDecodeError("expected object", text, 0)
    return data
