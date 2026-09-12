#!/usr/bin/env python3
"""Smoke test for mlx-lm server streaming."""

import os
from pathlib import Path

from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8001/v1", api_key="not-needed")
local = Path(__file__).resolve().parent.parent / "models" / "Qwen3-14B-4bit"
model = os.environ.get(
    "BUDDY_MODEL",
    str(local) if (local / "config.json").is_file() else "mlx-community/Qwen3-14B-4bit",
)

print("Sending test prompt to MLX server...")
stream = client.chat.completions.create(
    model=model,
    messages=[{"role": "user", "content": "Say hello in one short sentence."}],
    stream=True,
    max_tokens=64,
)

print("Response: ", end="", flush=True)
for chunk in stream:
    delta = chunk.choices[0].delta.content
    if delta:
        print(delta, end="", flush=True)
print("\nDone.")
