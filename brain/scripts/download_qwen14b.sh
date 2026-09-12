#!/usr/bin/env bash
# Resume-safe download of mlx-community/Qwen3-14B-4bit into brain/models/.
# 16 GB Macs: use this 14B-4bit (~8 GB weights). Do not use 30B-A3B.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST="${BUDDY_QWEN14_DIR:-$SCRIPT_DIR/../models/Qwen3-14B-4bit}"
BASE="https://huggingface.co/mlx-community/Qwen3-14B-4bit/resolve/main"
HF_ID="mlx-community/Qwen3-14B-4bit"

S1_NAME="model-00001-of-00002.safetensors"
S2_NAME="model-00002-of-00002.safetensors"
S1_SIZE=5354381380
S2_SIZE=2953517134
S1_SHA=5795efcfc7c96fd273e600562e8b111bfcc427415de9001d0a07e70cd99cff19
S2_SHA=2814562d654fe2d541fd4682804a0ccaa400e79701872c8e9f5998cf9481fdf8

SMALL_FILES=(
  config.json
  model.safetensors.index.json
  tokenizer.json
  tokenizer_config.json
  special_tokens_map.json
  added_tokens.json
  vocab.json
  merges.txt
  README.md
)

mkdir -p "$DEST"
cd "$DEST"

sha_of() {
  shasum -a 256 "$1" | awk '{print $1}'
}

fetch() {
  local name="$1"
  local expected="${2:-}"
  local expect_sha="${3:-}"
  if [[ -n "$expected" && -f "$name" ]]; then
    local have
    have="$(stat -f%z "$name" 2>/dev/null || stat -c%s "$name")"
    if [[ "$have" == "$expected" ]]; then
      if [[ -n "$expect_sha" ]]; then
        local got
        got="$(sha_of "$name")"
        if [[ "$got" == "$expect_sha" ]]; then
          echo "ok $name ($have bytes, sha256 match)"
          return 0
        fi
        echo "sha mismatch $name — deleting corrupt file" >&2
        rm -f "$name"
      else
        echo "ok $name ($have bytes)"
        return 0
      fi
    else
      echo "resume $name ($have / $expected)"
    fi
  else
    echo "get $name"
  fi
  curl -fL --http1.1 --retry 12 --retry-all-errors --retry-delay 3 -C - \
    -o "$name" "$BASE/$name"
  if [[ -n "$expected" ]]; then
    local have
    have="$(stat -f%z "$name" 2>/dev/null || stat -c%s "$name")"
    if [[ "$have" != "$expected" ]]; then
      echo "size mismatch $name: got $have want $expected" >&2
      exit 1
    fi
  fi
  if [[ -n "$expect_sha" ]]; then
    local got
    got="$(sha_of "$name")"
    if [[ "$got" != "$expect_sha" ]]; then
      echo "sha mismatch $name: got $got want $expect_sha" >&2
      exit 1
    fi
    echo "verified $name"
  fi
}

for f in "${SMALL_FILES[@]}"; do
  [[ -s "$DEST/$f" ]] || fetch "$f"
done

fetch "$S1_NAME" "$S1_SIZE" "$S1_SHA"
fetch "$S2_NAME" "$S2_SIZE" "$S2_SHA"

# Point HF hub cache at the local dir so --model mlx-community/Qwen3-14B-4bit
# does not re-download. Hub resolves `main` to the commit snapshot, not `local`.
HF_HOME="${HF_HOME:-$HOME/.cache/huggingface}"
CACHE="$HF_HOME/hub/models--mlx-community--Qwen3-14B-4bit"
REV="${BUDDY_QWEN14_REV:-a4d9b2df59d2c150bef02fcbe0d91046b7ca33a4}"
SNAP="$CACHE/snapshots/$REV"
mkdir -p "$SNAP" "$CACHE/refs"
ln -sfn "$DEST" "$CACHE/snapshots/local"
printf '%s\n' "$REV" > "$CACHE/refs/main"
for f in "$DEST"/*; do
  ln -sfn "$f" "$SNAP/$(basename "$f")"
done

echo "ready: $DEST"
echo "mlx: mlx_lm.server --model $DEST --host 127.0.0.1 --port 8001"
echo "or:  mlx_lm.server --model $HF_ID --host 127.0.0.1 --port 8001"
