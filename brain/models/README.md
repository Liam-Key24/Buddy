# Local models

Weights stay on disk and are gitignored.

| Directory | Role |
|-----------|------|
| `Qwen3-14B-4bit` | Buddy v1 model — keep until Qwen3.5 acceptance passes |
| `Qwen3.5-4B-4bit` | Intended production model (`mlx-community/Qwen3.5-4B-4bit`) |
| `.hf/` | Colocated Hugging Face cache for downloads (avoid a second `~/.cache` copy) |

Set `HF_HOME` / `HF_HUB_CACHE` under this folder when downloading.
