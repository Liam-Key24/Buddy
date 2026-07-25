#!/usr/bin/env bash
# Launch a built Buddy.app with this repo as BUDDY_PROJECT_ROOT.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export BUDDY_PROJECT_ROOT="$ROOT"

APP="${BUDDY_APP:-$ROOT/app/src-tauri/target/release/bundle/macos/Buddy.app}"

if [[ ! -d "$APP" ]]; then
  echo "Buddy.app not found at: $APP"
  echo "Build first:  cd app && npm run tauri build"
  echo "Or set BUDDY_APP=/path/to/Buddy.app"
  exit 1
fi

echo "BUDDY_PROJECT_ROOT=$BUDDY_PROJECT_ROOT"
echo "Opening $APP"
open "$APP"
