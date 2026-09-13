# Local model weights (inactive)

Buddy’s active architecture uses **Cloud AI (Groq)**, not local MLX/Qwen.

These folders may still exist on disk until Stage 12 cleanup after Groq acceptance:

- `Qwen3-14B-4bit`
- `Qwen3.5-4B-4bit`

Do not wire them back into the app. Remove only after the acceptance checklist in `docs/ACCEPTANCE.md` passes, by moving them to Trash (not an unrecoverable `rm -rf` of unrelated paths).
