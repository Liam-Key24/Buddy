"""Per-user settings API and calendar block sync."""

from __future__ import annotations

from pathlib import Path

from fastapi.testclient import TestClient

from app import main as mainmod
from app.control_plane import ControlPlane
from app.user_settings import DEFAULT_SETTINGS


def _client(tmp_path: Path) -> tuple[TestClient, ControlPlane]:
    plane = ControlPlane(db_path=tmp_path / "settings.db", ai=None)
    mainmod.plane.close()
    mainmod.plane = plane
    return TestClient(mainmod.app), plane


def _login(client: TestClient, username: str) -> None:
    client.cookies.clear()
    res = client.post(
        "/api/auth/login", json={"username": username, "password": "test-pass-ok"}
    )
    assert res.status_code == 200


def test_settings_defaults_and_full_time_work_blocks(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        _login(client, "liam")

        got = client.get("/api/settings")
        assert got.status_code == 200
        body = got.json()
        assert body["show_avatar"] is True
        assert body["work"]["mode"] == DEFAULT_SETTINGS["work"]["mode"]
        assert body["work"]["days"] == DEFAULT_SETTINGS["work"]["days"]
        assert body["prefer_after"] == "17:30"

        put = client.put(
            "/api/settings",
            json={
                "work": {
                    "enabled": True,
                    "mode": "full_time",
                    "start": "09:00",
                    "end": "17:00",
                    "days": [True, True, True, True, True, False, False],
                }
            },
        )
        assert put.status_code == 200
        assert put.json()["work"]["mode"] == "full_time"

        blocks = client.get("/api/calendar/fixed").json()
        work = [b for b in blocks if b["title"] == "Work"]
        assert len(work) == 5
        weekdays = sorted(b["weekday"] for b in work)
        assert weekdays == [0, 1, 2, 3, 4]
        for b in work:
            assert b["start_minute"] == 9 * 60
            assert b["end_minute"] == 17 * 60
    finally:
        plane.close()


def test_partner_part_time_shifts_isolated(tmp_path: Path):
    client, plane = _client(tmp_path)
    try:
        _login(client, "liam")
        client.put(
            "/api/settings",
            json={
                "work": {
                    "enabled": True,
                    "mode": "full_time",
                    "start": "09:00",
                    "end": "17:00",
                    "days": [True, True, True, True, True, False, False],
                }
            },
        )
        liam_blocks = client.get("/api/calendar/fixed").json()
        liam_work = [b for b in liam_blocks if b["title"] == "Work"]
        assert len(liam_work) == 5
        liam_ids = {b["id"] for b in liam_work}

        _login(client, "partner")
        # 2026-10-05 is a Monday
        put = client.put(
            "/api/settings",
            json={
                "work": {
                    "enabled": True,
                    "mode": "part_time",
                    "shifts": [
                        {
                            "id": "s1",
                            "date": "2026-10-05",
                            "start": "10:00",
                            "end": "14:00",
                            "place": "Cafe",
                        }
                    ],
                }
            },
        )
        assert put.status_code == 200
        assert put.json()["work"]["mode"] == "part_time"

        partner_blocks = client.get("/api/calendar/fixed").json()
        partner_work = [
            b
            for b in partner_blocks
            if b["title"] == "Work" or b["title"].startswith("Work · ")
        ]
        assert len(partner_work) == 1
        assert partner_work[0]["title"] == "Work · Cafe"
        assert partner_work[0]["weekday"] == 0
        assert partner_work[0]["start_minute"] == 10 * 60
        assert partner_work[0]["end_minute"] == 14 * 60
        assert partner_work[0]["id"] not in liam_ids

        _login(client, "liam")
        still = client.get("/api/calendar/fixed").json()
        still_work = [b for b in still if b["title"] == "Work"]
        assert len(still_work) == 5
        assert {b["id"] for b in still_work} == liam_ids
        assert all(b["title"] != "Work · Cafe" for b in still)
    finally:
        plane.close()
