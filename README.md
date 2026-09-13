# Buddy

Buddy turns unclear goals into realistic actions in its calendar, tracks what actually happens, and helps adjust the plan through natural conversation, with the user retaining final control.

## Experience

Four primary destinations:

- **Today** — agreed sessions, active goals, items needing attention
- **Chat** — natural discussion that owns goal intake
- **Calendar** — fixed, flexible, proposed, completed, and missed sessions
- **Sparks** — ideas without turning them into commitments

## Stack

- `frontend/` — React, TypeScript, Vite, FullCalendar
- `backend/` — FastAPI, SQLite, one MLX model integration
- `brain/models/` — local model weights (gitignored)

One frontend, one backend, one database, one model.

## Models

Preserved on disk (not in Git):

- `brain/models/Qwen3-14B-4bit` — Buddy v1 model, keep until Qwen3.5 acceptance passes
- `brain/models/Qwen3.5-4B-4bit` — `mlx-community/Qwen3.5-4B-4bit` @ `0e7ffd5c…` (~3 GB)
- Download cache colocated at `brain/models/.hf` (do not use a second `~/.cache` copy)

## Development

```bash
# Backend
cd backend
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
uvicorn app.main:app --reload --port 8787

# Frontend
cd frontend
npm install
npm run dev
```

## Personal data

Buddy v1 user data lives outside this repo at:

`~/Library/Application Support/com.liamgk.buddy/`

It is preserved until migration or deletion is explicitly agreed.
