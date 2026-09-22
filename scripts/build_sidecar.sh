#!/usr/bin/env bash
# Build a PyInstaller sidecar binary for the Buddy FastAPI backend.
# Output: src-tauri/sidecars/buddy-backend-<target-triple>
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/backend"
# shellcheck disable=SC1091
source .venv/bin/activate
pip install -q pyinstaller
TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
OUT_DIR="$ROOT/src-tauri/sidecars"
mkdir -p "$OUT_DIR"
export BUDDY_SIDECAR_TRIPLE="$TRIPLE"

pyinstaller \
  --noconfirm \
  --clean \
  --distpath "$OUT_DIR" \
  --workpath /tmp/buddy-sidecar-build \
  "$ROOT/backend/buddy-backend.spec"

echo "Built $OUT_DIR/buddy-backend-${TRIPLE}"
echo "Store GROQ_API_KEY in Keychain:"
echo "  security add-generic-password -a buddy -s com.liamgk.buddy.groq -w 'YOUR_KEY'"
