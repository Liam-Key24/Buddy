#!/usr/bin/env bash
# Build a PyInstaller sidecar binary for the Buddy FastAPI backend.
# Output: src-tauri/sidecars/buddy-backend-<target-triple>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/backend"
source .venv/bin/activate
pip install -q pyinstaller
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
OUT_DIR="$ROOT/src-tauri/sidecars"
mkdir -p "$OUT_DIR"
NAME="buddy-backend-${TRIPLE}"

# Entry module launches uvicorn bound to localhost.
cat > /tmp/buddy_sidecar_entry.py <<'PY'
import os
import uvicorn

if __name__ == "__main__":
    host = os.environ.get("BUDDY_HOST", "127.0.0.1")
    port = int(os.environ.get("BUDDY_PORT", "8787"))
    uvicorn.run("app.main:app", host=host, port=port, log_level="info")
PY

pyinstaller \
  --noconfirm \
  --clean \
  --onefile \
  --name "$NAME" \
  --distpath "$OUT_DIR" \
  --workpath /tmp/buddy-sidecar-build \
  --paths "$ROOT/backend" \
  /tmp/buddy_sidecar_entry.py

echo "Built $OUT_DIR/$NAME"
echo "Store GROQ_API_KEY in Keychain:"
echo "  security add-generic-password -a buddy -s com.liamgk.buddy.groq -w 'YOUR_KEY'"
