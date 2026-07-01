#!/usr/bin/env python3
"""Smoke test for mlx-lm server streaming."""

from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8001/v1", api_key="not-needed")

print("Sending test prompt to MLX server...")
stream = client.chat.completions.create(
    model="mlx-community/Llama-3.2-3B-Instruct-4bit",
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
