#!/usr/bin/env python3
"""SQLite online backup + integrity check. Never logs message content or secrets."""

from __future__ import annotations

import argparse
import os
import sqlite3
import sys
from datetime import datetime, timezone
from pathlib import Path


def _ts() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def backup(src: Path, dest_dir: Path, *, keep: int) -> Path:
    if not src.is_file():
        raise SystemExit(f"source database missing: {src}")
    dest_dir.mkdir(parents=True, exist_ok=True)
    dest = dest_dir / f"buddy-{_ts()}.db"
    src_conn = sqlite3.connect(str(src))
    try:
        dst_conn = sqlite3.connect(str(dest))
        try:
            with dst_conn:
                src_conn.backup(dst_conn)
        finally:
            dst_conn.close()
    finally:
        src_conn.close()

    check = sqlite3.connect(str(dest))
    try:
        row = check.execute("PRAGMA integrity_check").fetchone()
        if not row or row[0] != "ok":
            raise SystemExit(f"backup integrity failed: {row}")
    finally:
        check.close()

    os.chmod(dest, 0o600)
    _rotate(dest_dir, keep=keep)
    print(f"ok backup={dest} bytes={dest.stat().st_size}")
    return dest


def _rotate(dest_dir: Path, *, keep: int) -> None:
    if keep < 1:
        return
    files = sorted(dest_dir.glob("buddy-*.db"), key=lambda p: p.name)
    # Never delete the last known-good file even if keep==1 and we just wrote one.
    while len(files) > keep:
        if len(files) <= 1:
            break
        victim = files.pop(0)
        victim.unlink(missing_ok=True)
        print(f"rotated_out={victim.name}")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument(
        "--db",
        default=os.environ.get("BUDDY_DB_PATH", "/var/lib/buddy/buddy.db"),
        help="Source SQLite path",
    )
    p.add_argument(
        "--dir",
        default=os.environ.get("BUDDY_BACKUP_DIR", "/var/lib/buddy/backups"),
        help="Backup directory",
    )
    p.add_argument("--keep", type=int, default=int(os.environ.get("BUDDY_BACKUP_KEEP", "14")))
    args = p.parse_args()
    backup(Path(args.db).expanduser(), Path(args.dir).expanduser(), keep=args.keep)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as exc:  # noqa: BLE001 — CLI boundary
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
