"""Test Brain /embed endpoint."""
import sys

import httpx

BRAIN_URL = "http://127.0.0.1:8002"


def main() -> int:
    text = "When did we decide to make Buddy local-first?"
    try:
        resp = httpx.post(f"{BRAIN_URL}/embed", json={"text": text}, timeout=30.0)
        resp.raise_for_status()
        data = resp.json()
        embedding = data["embedding"]
        dimensions = data["dimensions"]
        print(f"dimensions={dimensions} len={len(embedding)}")
        assert len(embedding) == dimensions
        assert all(isinstance(v, float) for v in embedding)
        print("embed test passed")
        return 0
    except Exception as e:
        print(f"embed test failed: {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
