"""Cloud AI offline behaviour."""

from __future__ import annotations

from pathlib import Path

from app.config import DEFAULT_CORS_ORIGINS, Settings
from app.control_plane import ControlPlane


def test_offline_chat_explains_while_local_works(tmp_path: Path):
    settings = Settings(
        db_path=tmp_path / "off.db",
        host="127.0.0.1",
        port=8787,
        groq_api_key=None,
        groq_model="openai/gpt-oss-120b",
        groq_base_url="https://api.groq.com/openai/v1",
        ai_enabled=True,
        request_timeout_s=5,
        max_output_tokens=256,
        cors_origins=DEFAULT_CORS_ORIGINS,
        static_dir=None,
    )
    plane = ControlPlane(db_path=tmp_path / "off.db", settings=settings, ai=None)
    try:
        res = plane.handle_message("I want to climb V6")
        assert res.ai_available is False
        assert "cloud ai" in res.reply.lower()
        spark = plane.sparks.capture("later idea")
        assert spark.id
        today = plane.get_today()
        assert isinstance(today.goals, list)
    finally:
        plane.close()
