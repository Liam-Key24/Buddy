#!/usr/bin/env python3
"""Restore verification (default) or apply a backup over the live DB."""

from __future__ import annotations

import argparse
import os
import shutil
import sqlite3
import sys
import tempfile
from pathlib import Path


def integrity_ok(path: Path) -> str:
    conn = sqlite3.connect(str(path))
    try:
        row = conn.execute("PRAGMA integrity_check").fetchone()
        return row[0] if row else "missing"
    finally:
        conn.close()


def verify(backup: Path) -> None:
    if not backup.is_file():
        raise SystemExit(f"backup missing: {backup}")
    with tempfile.TemporaryDirectory(prefix="buddy-restore-") as tmp:
        dest = Path(tmp) / "buddy.db"
        shutil.copy2(backup, dest)
        result = integrity_ok(dest)
        if result != "ok":
            raise SystemExit(f"restore verification failed: {result}")
        print(f"ok verified={backup} integrity={result}")


def apply(backup: Path, live: Path) -> None:
    if not backup.is_file():
        raise SystemExit(f"backup missing: {backup}")
    result = integrity_ok(backup)
    if result != "ok":
        raise SystemExit(f"refusing apply; backup integrity={result}")
    live.parent.mkdir(parents=True, exist_ok=True)
    # Online-safe: copy to sibling then replace
    staging = live.with_suffix(live.suffix + ".restoring")
    shutil.copy2(backup, staging)
    if integrity_ok(staging) != "ok":
        staging.unlink(missing_ok=True)
        raise SystemExit("staging copy failed integrity check")
    os.replace(staging, live)
    for suffix in ("-wal", "-shm"):
        side = Path(str(live) + suffix)
        side.unlink(missing_ok=True)
    os.chmod(live, 0o600)
    print(f"ok applied={backup} live={live}")


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("backup", type=Path, help="Path to buddy-*.db backup")
    p.add_argument(
        "--db",
        default=os.environ.get("BUDDY_DB_PATH", "/var/lib/buddy/buddy.db"),
        help="Live database path (only used with --apply)",
    )
    p.add_argument(
        "--apply",
        action="store_true",
        help="Replace the live database (default is verify-only into a temp path)",
    )
    args = p.parse_args()
    backup = args.backup.expanduser()
    if args.apply:
        apply(backup, Path(args.db).expanduser())
    else:
        verify(backup)


if __name__ == "__main__":
    try:
        main()
    except SystemExit:
        raise
    except Exception as exc:  # noqa: BLE001
        print(f"error: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc
