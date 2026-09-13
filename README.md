# Buddy

Buddy turns unclear goals into realistic actions in its calendar, tracks what actually happens, and helps adjust the plan through natural conversation, with the user retaining final control.

## Experience

Four primary destinations:

- **Today** — agreed sessions, active goals, items needing attention
- **Chat** — natural discussion that owns goal intake
- **Calendar** — fixed, flexible, proposed, completed, and missed sessions
- **Sparks** — ideas without turning them into commitments

Settings stay secondary. The interface refers to language understanding as **Cloud AI** (Groq).

## Stack

- `frontend/` — React, TypeScript, Vite, FullCalendar
- `backend/` — FastAPI on `127.0.0.1`, SQLite, one Groq provider
- One control plane: user message → one Groq request → validated `BuddyTurn` → deterministic handlers

## Cloud AI setup

1. Create a Groq API key.
2. Enable **Zero Data Retention** in the Groq console for that key (Buddy cannot verify ZDR via API).
3. Copy `backend/.env.example` to `backend/.env` and set:

```bash
GROQ_API_KEY=...
GROQ_MODEL=openai/gpt-oss-120b   # default; override only if your account exposes another model
BUDDY_AI_ENABLED=1
BUDDY_DB_PATH=                    # optional; defaults to backend/data/buddy.db
```

Retired Llama free-tier IDs must not be hardcoded. If `GROQ_MODEL` is unavailable, Buddy returns a configuration error and does not silently switch models.

When Cloud AI is offline, Today / Calendar / Sparks keep working; Chat explains that conversational understanding is temporarily unavailable.

## Development

```bash
# Backend
cd backend
python3 -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
set -a && source .env && set +a   # if present
PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787 --reload

# Frontend
cd frontend
npm install
npm run dev
```

## Database

Development uses `backend/data/buddy.db` (gitignored). TablePlus is a viewer for this file.

Packaged builds should set `BUDDY_DB_PATH` to a stable Application Support location. Never store the writable database inside the signed app bundle.

## Personal data

Buddy v1 user data may still exist at:

`~/Library/Application Support/com.liamgk.buddy/`

It is preserved until migration or deletion is explicitly agreed.

Local Qwen/MLX weights under `brain/models/` are no longer part of the active architecture; remove them only after Groq acceptance passes.
