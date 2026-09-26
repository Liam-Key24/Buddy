"""PyInstaller entry for the packaged Buddy backend sidecar."""

from __future__ import annotations

import os

import uvicorn

import app.main as buddy_app


if __name__ == "__main__":
    host = os.environ.get("BUDDY_HOST", "127.0.0.1")
    port = int(os.environ.get("BUDDY_PORT", "8787"))
    uvicorn.run(buddy_app.app, host=host, port=port, log_level="info")
