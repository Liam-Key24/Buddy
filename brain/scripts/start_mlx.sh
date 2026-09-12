#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VENV_DIR="$SCRIPT_DIR/../venv"
LOCAL_MODEL="$SCRIPT_DIR/../models/Qwen3-14B-4bit"
HF_MODEL="mlx-community/Qwen3-14B-4bit"
S1_SIZE=5354381380
S2_SIZE=2953517134

if [[ -f "$VENV_DIR/bin/activate" ]]; then
  source "$VENV_DIR/bin/activate"
fi

model_ready() {
  local dir="$1"
  [[ -f "$dir/config.json" ]] || return 1
  [[ -f "$dir/model-00001-of-00002.safetensors" ]] || return 1
  [[ -f "$dir/model-00002-of-00002.safetensors" ]] || return 1
  local s1 s2
  s1="$(stat -f%z "$dir/model-00001-of-00002.safetensors" 2>/dev/null || stat -c%s "$dir/model-00001-of-00002.safetensors")"
  s2="$(stat -f%z "$dir/model-00002-of-00002.safetensors" 2>/dev/null || stat -c%s "$dir/model-00002-of-00002.safetensors")"
  [[ "$s1" == "$S1_SIZE" && "$s2" == "$S2_SIZE" ]]
}

if [[ -n "${BUDDY_MODEL:-}" ]]; then
  MODEL="$BUDDY_MODEL"
elif model_ready "$LOCAL_MODEL"; then
  MODEL="$LOCAL_MODEL"
else
  MODEL="$HF_MODEL"
fi

exec mlx_lm.server \
  --model "$MODEL" \
  --host 127.0.0.1 \
  --port 8001
