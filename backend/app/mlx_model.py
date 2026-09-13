"""Single local MLX model integration for Buddy."""

from __future__ import annotations

import os
from pathlib import Path
from typing import Any

MODELS_ROOT = Path(__file__).resolve().parents[2] / "brain" / "models"
PREFERRED = MODELS_ROOT / "Qwen3.5-4B-4bit"
FALLBACK = MODELS_ROOT / "Qwen3-14B-4bit"

_model = None
_tokenizer = None
_load_error: str | None = None


def resolve_model_path() -> Path | None:
    env = os.environ.get("BUDDY_MODEL_PATH")
    if env:
        p = Path(env)
        if p.exists():
            return p
    if PREFERRED.exists() and any(PREFERRED.glob("*.safetensors")):
        return PREFERRED
    if FALLBACK.exists() and any(FALLBACK.glob("*.safetensors")):
        return FALLBACK
    return None


def model_status() -> dict[str, Any]:
    path = resolve_model_path()
    return {
        "path": str(path) if path else None,
        "preferred_exists": PREFERRED.exists(),
        "fallback_exists": FALLBACK.exists(),
        "loaded": _model is not None,
        "load_error": _load_error,
    }


def load_model() -> bool:
    """Lazy-load the local MLX model. Returns False if unavailable."""
    global _model, _tokenizer, _load_error
    if _model is not None:
        return True
    path = resolve_model_path()
    if path is None:
        _load_error = "No local model found under brain/models"
        return False
    try:
        from mlx_lm import load

        _model, _tokenizer = load(str(path))
        _load_error = None
        return True
    except Exception as exc:  # noqa: BLE001 — surface any load failure
        _load_error = str(exc)
        _model = None
        _tokenizer = None
        return False


def generate_reply(system: str, user: str, max_tokens: int = 256) -> str | None:
    if not load_model():
        return None
    assert _tokenizer is not None and _model is not None
    from mlx_lm import generate

    messages = [
        {"role": "system", "content": system},
        {"role": "user", "content": user},
    ]
    prompt = user
    if hasattr(_tokenizer, "apply_chat_template"):
        prompt = _tokenizer.apply_chat_template(
            messages,
            add_generation_prompt=True,
            tokenize=False,
        )
    return generate(_model, _tokenizer, prompt=prompt, max_tokens=max_tokens, verbose=False)
