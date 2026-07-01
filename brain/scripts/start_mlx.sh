#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/../venv"

if [[ -f "$VENV_DIR/bin/activate" ]]; then
  source "$VENV_DIR/bin/activate"
fi

exec mlx_lm.server \
  --model mlx-community/Llama-3.2-3B-Instruct-4bit \
  --host 127.0.0.1 \
  --port 8001
