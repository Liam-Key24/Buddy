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
    rate_limit: int | None = None
    rate_remaining: int | None = None
    rate_reset: str | None = None


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

    def _client_headers(self) -> dict[str, str]:
        return {
            "Authorization": f"Bearer {self.settings.groq_api_key}",
            "Content-Type": "application/json",
        }

    def _http(self) -> httpx.Client:
        if self._client is None:
            self._client = httpx.Client(
                base_url=self.settings.groq_base_url,
                timeout=self.settings.request_timeout_s,
                headers=self._client_headers(),
            )
        return self._client

    def open_request_client(self) -> httpx.Client:
        """Dedicated client for one cancellable request. Caller must close it."""
        return httpx.Client(
            base_url=self.settings.groq_base_url,
            timeout=self.settings.request_timeout_s,
            headers=self._client_headers(),
        )

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

    def complete_json(
        self,
        system: str,
        user: str,
        *,
        allow_retry: bool = True,
        cancel_check=None,
        http_client: httpx.Client | None = None,
    ) -> dict[str, Any]:
        """One physical Groq POST. HTTP/transport errors are not retried here."""
        del allow_retry
        if cancel_check and cancel_check():
            raise GroqError("cancelled", "Stopped")
        self.validate_model()
        if cancel_check and cancel_check():
            raise GroqError("cancelled", "Stopped")
        payload = {
            "model": self.settings.groq_model,
            "temperature": 0.2,
            "max_completion_tokens": self.settings.max_output_tokens,
            "max_tokens": self.settings.max_output_tokens,
            "reasoning_effort": "low",
            "include_reasoning": True,
            "messages": [
                {"role": "system", "content": system},
                {"role": "user", "content": user},
            ],
        }
        return self._request_json(
            payload, retried=False, cancel_check=cancel_check, http_client=http_client
        )

    def _request_json(
        self,
        payload: dict[str, Any],
        *,
        retried: bool,
        cancel_check=None,
        http_client: httpx.Client | None = None,
    ) -> dict[str, Any]:
        started = time.perf_counter()
        status: int | None = None
        client = http_client or self._http()
        try:
            if cancel_check and cancel_check():
                raise GroqError("cancelled", "Stopped")
            resp = client.post("/chat/completions", json=payload)
            status = resp.status_code
            if status == 429:
                raise GroqError("rate_limit", "Cloud AI is rate-limited right now")
            if status >= 500:
                raise GroqError("http", f"Cloud AI server error ({status})")
            if status >= 400:
                kind = _client_error_kind(resp)
                if kind == "malformed":
                    raise GroqError("malformed", "Cloud AI returned unusable JSON")
                raise GroqError("http", f"Cloud AI request failed ({status})")
            body = resp.json()
            message = body["choices"][0]["message"]
            content = _message_text(message)
            usage = body.get("usage") or {}
            rate_limit, rate_remaining, rate_reset = _rate_headers(resp)
            self.last_stats = GroqCallStats(
                latency_ms=int((time.perf_counter() - started) * 1000),
                status=status,
                tokens_prompt=usage.get("prompt_tokens"),
                tokens_completion=usage.get("completion_tokens"),
                retried=retried,
                rate_limit=rate_limit,
                rate_remaining=rate_remaining,
                rate_reset=rate_reset,
            )
            log.info(
                "groq_ok latency_ms=%s status=%s prompt_tokens=%s completion_tokens=%s content_len=%s retried=%s",
                self.last_stats.latency_ms,
                status,
                self.last_stats.tokens_prompt,
                self.last_stats.tokens_completion,
                len(content or ""),
                retried,
            )
            try:
                return _parse_json_object(content)
            except json.JSONDecodeError:
                log.warning(
                    "groq_err category=malformed latency_ms=%s status=%s content_len=%s keys=%s retried=%s",
                    self.last_stats.latency_ms,
                    status,
                    len(content or ""),
                    sorted(message.keys()) if isinstance(message, dict) else [],
                    retried,
                )
                raise GroqError("malformed", "Cloud AI returned unusable JSON") from None
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
            log.warning(
                "groq_err category=malformed latency_ms=%s status=%s retried=%s",
                self.last_stats.latency_ms,
                status,
                retried,
            )
            raise GroqError("malformed", "Cloud AI returned unusable output") from exc
        except httpx.HTTPError as exc:
            category = "cancelled" if (cancel_check and cancel_check()) else "transport"
            self.last_stats = GroqCallStats(
                latency_ms=int((time.perf_counter() - started) * 1000),
                status=status,
                retried=retried,
                error_category=category,
            )
            if category == "cancelled":
                raise GroqError("cancelled", "Stopped") from exc
            raise GroqError("transport", f"Cloud AI unreachable ({exc.__class__.__name__})") from exc


def _client_error_kind(resp: httpx.Response) -> str:
    """Classify a 4xx without logging provider content."""
    try:
        payload = resp.json()
    except Exception:
        return "http"
    err = payload.get("error") if isinstance(payload, dict) else None
    if not isinstance(err, dict):
        return "http"
    code = str(err.get("code") or "")
    err_type = str(err.get("type") or "")
    message = str(err.get("message") or "").lower()
    if (
        code in {"json_validate_failed", "failed_generation"}
        or "json_validate_failed" in err_type
        or "json_validate_failed" in message
        or "failed to validate json" in message
        or "failed_generation" in message
    ):
        return "malformed"
    return "http"


def _message_text(message: dict[str, Any]) -> str:
    content = message.get("content")
    if isinstance(content, list):
        parts: list[str] = []
        for item in content:
            if isinstance(item, str):
                parts.append(item)
            elif isinstance(item, dict):
                parts.append(str(item.get("text") or ""))
        content = "".join(parts)
    text = str(content or "").strip()
    if text:
        return text
    for key in ("reasoning", "reasoning_content"):
        extra = message.get(key)
        if isinstance(extra, str) and extra.strip():
            return extra.strip()
    return text


def _rate_headers(resp: httpx.Response) -> tuple[int | None, int | None, str | None]:
    def _int(name: str) -> int | None:
        raw = resp.headers.get(name)
        if raw is None:
            return None
        try:
            return int(raw)
        except ValueError:
            return None

    limit = _int("x-ratelimit-limit-requests") or _int("x-ratelimit-limit-tokens")
    remaining = _int("x-ratelimit-remaining-requests") or _int("x-ratelimit-remaining-tokens")
    reset = resp.headers.get("x-ratelimit-reset-requests") or resp.headers.get("x-ratelimit-reset-tokens")
    return limit, remaining, reset


def _parse_json_object(text: str) -> dict[str, Any]:
    text = (text or "").strip()
    if text.startswith("```"):
        text = text.removeprefix("```json").removeprefix("```").removesuffix("```").strip()
    try:
        data = json.loads(text)
        if isinstance(data, dict):
            return data
    except json.JSONDecodeError:
        data = None
    decoder = json.JSONDecoder()
    idx = 0
    fallback: dict[str, Any] | None = None
    while True:
        start = text.find("{", idx)
        if start < 0:
            break
        try:
            data, _end = decoder.raw_decode(text[start:])
        except json.JSONDecodeError:
            idx = start + 1
            continue
        if isinstance(data, dict):
            if any(key in data for key in ("assistant_text", "operations", "intents")):
                return data
            fallback = data if fallback is None else fallback
        idx = start + 1
    if fallback is not None:
        return fallback
    raise json.JSONDecodeError("expected object", text, 0)
