"""Buddy runtime configuration. Secrets never leave the process."""

from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from dotenv import load_dotenv

_BACKEND_ROOT = Path(__file__).resolve().parents[1]
# Load backend/.env for local/dev. Never commit this file.
load_dotenv(_BACKEND_ROOT / ".env", override=False)

DEFAULT_DB = _BACKEND_ROOT / "data" / "buddy.db"
DEFAULT_GROQ_BASE = "https://api.groq.com/openai/v1"
DEFAULT_GROQ_MODEL = "openai/gpt-oss-120b"
DEFAULT_CORS_ORIGINS = (
    "http://localhost:5173",
    "http://127.0.0.1:5173",
    "tauri://localhost",
    "https://tauri.localhost",
)


def _env_bool(name: str, default: bool = False) -> bool:
    raw = os.environ.get(name)
    if raw is None:
        return default
    return raw.strip().lower() in {"1", "true", "yes", "on"}


def _env_float(name: str, default: float) -> float:
    raw = os.environ.get(name)
    if raw is None or not raw.strip():
        return default
    try:
        return float(raw)
    except ValueError:
        return default


def _env_cors(default: tuple[str, ...]) -> tuple[str, ...]:
    raw = os.environ.get("BUDDY_CORS_ORIGINS", "").strip()
    if not raw:
        return default
    extras = [part.strip() for part in raw.split(",") if part.strip()]
    return tuple(dict.fromkeys([*default, *extras]))


def _env_int(name: str, default: int) -> int:
    raw = os.environ.get(name)
    if raw is None or not raw.strip():
        return default
    try:
        return int(raw)
    except ValueError:
        return default


@dataclass(frozen=True)
class Settings:
    db_path: Path
    host: str
    port: int
    groq_api_key: str | None
    groq_model: str
    groq_base_url: str
    ai_enabled: bool
    request_timeout_s: float
    max_output_tokens: int
    cors_origins: tuple[str, ...]

    @property
    def groq_configured(self) -> bool:
        return bool(self.groq_api_key and self.ai_enabled)


def load_settings() -> Settings:
    db_raw = os.environ.get("BUDDY_DB_PATH", "").strip()
    db_path = Path(db_raw).expanduser() if db_raw else DEFAULT_DB
    return Settings(
        db_path=db_path,
        host=os.environ.get("BUDDY_HOST", "127.0.0.1").strip() or "127.0.0.1",
        port=_env_int("BUDDY_PORT", 8787),
        groq_api_key=os.environ.get("GROQ_API_KEY", "").strip() or None,
        groq_model=os.environ.get("GROQ_MODEL", DEFAULT_GROQ_MODEL).strip() or DEFAULT_GROQ_MODEL,
        groq_base_url=(os.environ.get("GROQ_BASE_URL", DEFAULT_GROQ_BASE).strip() or DEFAULT_GROQ_BASE).rstrip(
            "/"
        ),
        # Default on when a key is present; set BUDDY_AI_ENABLED=0 to force local-only.
        ai_enabled=_env_bool("BUDDY_AI_ENABLED", default=True),
        request_timeout_s=_env_float("BUDDY_GROQ_TIMEOUT_S", 45.0),
        max_output_tokens=_env_int("BUDDY_GROQ_MAX_TOKENS", 1024),
        cors_origins=_env_cors(DEFAULT_CORS_ORIGINS),
    )
