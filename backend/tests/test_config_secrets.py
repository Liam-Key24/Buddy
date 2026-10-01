"""GROQ_API_KEY loading: injected secrets win, blank placeholders do not."""

from __future__ import annotations

import os
from pathlib import Path

from dotenv import load_dotenv

from app.config import _prefer_injected_secret


def test_blank_env_does_not_hide_dotenv_key(tmp_path: Path, monkeypatch):
    monkeypatch.setenv("GROQ_API_KEY", "   ")
    kept = _prefer_injected_secret("GROQ_API_KEY")
    assert kept == ""
    assert "GROQ_API_KEY" not in os.environ

    env_file = tmp_path / ".env"
    env_file.write_text("GROQ_API_KEY=from-file\n", encoding="utf-8")
    load_dotenv(env_file, override=False)
    assert os.environ["GROQ_API_KEY"] == "from-file"


def test_injected_secret_wins_over_blank_dotenv_line(tmp_path: Path, monkeypatch):
    monkeypatch.setenv("GROQ_API_KEY", "injected-secret")
    kept = _prefer_injected_secret("GROQ_API_KEY")
    assert kept == "injected-secret"

    env_file = tmp_path / ".env"
    env_file.write_text("GROQ_API_KEY=\n", encoding="utf-8")
    load_dotenv(env_file, override=False)
    if kept:
        os.environ["GROQ_API_KEY"] = kept
    assert os.environ["GROQ_API_KEY"] == "injected-secret"
