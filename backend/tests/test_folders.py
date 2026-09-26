"""Chat folders use zero Cloud AI requests."""

from __future__ import annotations

from pathlib import Path

from app.config import DEFAULT_CORS_ORIGINS, Settings
from app.control_plane import ControlPlane


def _plane(tmp_path: Path) -> ControlPlane:
    settings = Settings(
        db_path=tmp_path / "f.db",
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
    return ControlPlane(db_path=tmp_path / "f.db", settings=settings, ai=None)


def test_folder_crud_and_move_zero_ai(tmp_path: Path):
    plane = _plane(tmp_path)
    try:
        before_ai = plane.conn.execute("SELECT COUNT(*) AS c FROM ai_requests").fetchone()["c"]
        folder = plane.create_folder("Climbing")
        assert folder["title"] == "Climbing"
        renamed = plane.rename_folder(folder["id"], "Projects")
        assert renamed["title"] == "Projects"
        chat = plane.create_conversation()
        moved = plane.move_conversation(chat["id"], folder["id"])
        assert moved["folder_id"] == folder["id"]
        listed = plane.list_folders()
        assert len(listed) == 1
        deleted = plane.delete_folder(folder["id"])
        assert deleted["id"] == folder["id"]
        remaining = plane.conversations.get(chat["id"])
        assert remaining["folder_id"] is None
        assert plane.list_folders() == []
        other = plane.create_conversation()
        folder = plane.create_folder("Later")
        placed = plane.place_conversation(other["id"], folder["id"], None)
        assert placed["folder_id"] == folder["id"]
        inserted = plane.place_conversation(chat["id"], folder["id"], other["id"])
        ordered = [c for c in plane.list_conversations() if c["folder_id"] == folder["id"]]
        assert [c["id"] for c in ordered] == [chat["id"], other["id"]]
        assert inserted["sort_order"] == 0
        missing = plane.move_conversation(chat["id"], "no-such-folder")
        assert missing is None
        after = plane.conn.execute("SELECT COUNT(*) AS c FROM ai_requests").fetchone()["c"]
        assert before_ai == after == 0
    finally:
        plane.close()
