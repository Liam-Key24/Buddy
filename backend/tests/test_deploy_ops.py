"""Backup / restore helpers used by deploy scripts."""

from __future__ import annotations

import sqlite3
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BACKUP = ROOT / "deploy" / "backup_buddy.py"
RESTORE = ROOT / "deploy" / "restore_buddy.py"


def _seed(db: Path) -> None:
    conn = sqlite3.connect(str(db))
    conn.execute("CREATE TABLE t (id INTEGER PRIMARY KEY, note TEXT)")
    conn.execute("INSERT INTO t (note) VALUES ('hello')")
    conn.commit()
    conn.close()


def test_backup_and_verify(tmp_path: Path):
    src = tmp_path / "buddy.db"
    out = tmp_path / "backups"
    _seed(src)
    subprocess.run(
        [sys.executable, str(BACKUP), "--db", str(src), "--dir", str(out), "--keep", "3"],
        check=True,
        cwd=str(ROOT),
    )
    files = list(out.glob("buddy-*.db"))
    assert len(files) == 1
    subprocess.run(
        [sys.executable, str(RESTORE), str(files[0])],
        check=True,
        cwd=str(ROOT),
    )


def test_health_is_sanitised(tmp_path: Path):
    from fastapi.testclient import TestClient

    from app import main as mainmod
    from app.control_plane import ControlPlane

    plane = ControlPlane(db_path=tmp_path / "health.db", ai=None)
    mainmod.plane = plane
    try:
        client = TestClient(mainmod.app)
        res = client.get("/api/health")
        assert res.status_code == 200
        body = res.json()
        assert body["ok"] is True
        assert body["database"] in {"available", "unavailable"}
        assert body["ai"] in {"configured", "unconfigured"}
        assert "db" not in body
        assert "host" not in body
        assert "model_configured" not in body
    finally:
        plane.close()
