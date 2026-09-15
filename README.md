# Buddy

Buddy turns unclear goals into realistic actions in its calendar, tracks what actually happens, and helps adjust the plan through natural conversation, with the user retaining final control.

This README is written for **local development**, **debugging**, and **deploying the web app** (Vercel frontend + hosted backend).

---

## What you get

| Page | Route | What it does |
|------|-------|--------------|
| **Today** | `/` | Live clock, usage stats, upcoming sessions, goals needing attention, progress bars |
| **Chat** | `/chat` | Goal intake, clarifications, calendar proposals, conversation sidebar |
| **Goals** | `/goals` | List goals, remove with confirm, “Edit in Chat” handoff |
| **Calendar** | `/calendar` | Week/day/month views, category filters, manual add/delete events |
| **Sparks** | `/sparks` | Capture ideas without committing; promote to Chat |
| **Settings** | `/settings` | Backend health, build info, how to start locally |

**Cloud AI** = Groq. Chat needs a valid `GROQ_API_KEY` on the backend. Today, Calendar, Goals, and Sparks still load when the backend is up but Groq is unavailable.

---

## Architecture (simple)

```text
Browser (React/Vite)
    │
    │  fetch(API_BASE + "/chat", "/today", …)
    ▼
FastAPI backend (:8787)
    │
    ├── SQLite (buddy.db) — goals, sessions, conversations, categories, sparks
    │
    └── Groq API — one request per chat turn → validated BuddyTurn → handlers
```

**One turn flow (Chat):**

1. User sends message → `POST /chat`
2. Backend loads conversation + active goal context
3. Single Groq call returns structured `BuddyTurn` (JSON)
4. Deterministic Python handlers update DB (goal fields, proposals, etc.)
5. Response includes assistant text, optional proposal card, clarifications

The frontend never sees your Groq key. Only the backend calls Groq.

---

## Repository layout

```text
BUDDY/
├── frontend/                 # React + TypeScript + Vite
│   ├── src/
│   │   ├── api.ts            # All HTTP calls; API_BASE from VITE_API_BASE
│   │   ├── AppShell.tsx      # Nav rail + routes outlet
│   │   ├── pages/            # Today, Chat, Goals, Calendar, Sparks, Settings
│   │   ├── components/       # ConfirmDialog, ProposalCards, etc.
│   │   └── index.css         # Shared shell + page styles
│   ├── vite.config.ts        # Dev proxy: /api → localhost:8787
│   └── vercel.json           # SPA rewrites for React Router on Vercel
│
├── backend/
│   ├── app/
│   │   ├── main.py           # FastAPI routes + CORS
│   │   ├── config.py         # Env loading (.env)
│   │   ├── control_plane.py  # Chat turn orchestration
│   │   ├── calendar.py       # Sessions, proposals, outcomes
│   │   ├── categories.py     # Category store + keyword matching
│   │   ├── goals.py          # Goal list/remove
│   │   ├── conversations.py  # Conversation CRUD + messages
│   │   ├── db.py             # SQLite connection
│   │   └── migrations.py     # Schema versions (v3 = categories)
│   ├── tests/                # pytest suite
│   ├── data/buddy.db         # Default DB (gitignored)
│   └── .env.example          # Copy to .env — never commit .env
│
├── docs/
│   └── ACCEPTANCE.md         # Manual product walkthrough
└── src-tauri/                # macOS desktop packaging (optional)
```

---

## Prerequisites

- **Python 3.11+** (backend)
- **Node 20+** and npm (frontend)
- **Groq API key** for Chat ([console.groq.com](https://console.groq.com))
- Optional: **TablePlus** or `sqlite3` CLI to inspect `buddy.db`

---

## Local development (step by step)

### 1. Backend

```bash
cd backend
python3 -m venv .venv
source .venv/bin/activate          # Windows: .venv\Scripts\activate
pip install -r requirements.txt
cp .env.example .env               # then edit .env
```

Minimum `.env`:

```bash
GROQ_API_KEY=gsk_...               # required for Chat
GROQ_MODEL=openai/gpt-oss-120b
BUDDY_AI_ENABLED=1
```

Start the API:

```bash
set -a && source .env && set +a    # load env into shell
PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787 --reload
```

**Sanity check:**

```bash
curl http://127.0.0.1:8787/health
# → {"ok":true,"product":"Buddy",...}
```

### 2. Frontend

```bash
cd frontend
npm install
npm run dev
```

Open **http://127.0.0.1:5173**

In dev, the frontend calls **`/api/...`**. Vite proxies that to `http://127.0.0.1:8787/...` (see `frontend/vite.config.ts`). You do **not** need `VITE_API_BASE` locally.

### 3. Disposable DB (optional)

Use a temp database so you do not touch your real data:

```bash
export BUDDY_DB_PATH=/tmp/buddy-dev.db
PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787 --reload
```

---

## Environment variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `GROQ_API_KEY` | — | Groq secret; backend only |
| `GROQ_MODEL` | `openai/gpt-oss-120b` | Model ID; Buddy errors if unavailable (no silent fallback) |
| `GROQ_BASE_URL` | `https://api.groq.com/openai/v1` | Override Groq endpoint |
| `BUDDY_AI_ENABLED` | `1` | Set `0` to disable Groq calls |
| `BUDDY_DB_PATH` | `backend/data/buddy.db` | SQLite file path |
| `BUDDY_HOST` | `127.0.0.1` | Bind address; use `0.0.0.0` in Docker/Railway/Render |
| `BUDDY_PORT` | `8787` | Listen port |
| `BUDDY_CORS_ORIGINS` | localhost + Tauri | Comma-separated browser origins, e.g. `https://buddy.vercel.app,http://localhost:5173` |
| `BUDDY_GROQ_TIMEOUT_S` | `45` | Groq request timeout |
| `BUDDY_GROQ_MAX_TOKENS` | `1024` | Max tokens per Groq response |

**Frontend (build time only):**

| Variable | Default | Purpose |
|----------|---------|---------|
| `VITE_API_BASE` | `/api` | Backend URL in production, e.g. `https://buddy-api.railway.app` — **no trailing slash**, no `/api` suffix |

Enable **Zero Data Retention** for your Groq key in the Groq console. Buddy cannot verify ZDR via API.

---

## API reference (quick)

All routes are on the backend root (not under `/api` unless you put a reverse proxy in front).

| Method | Path | Purpose |
|--------|------|---------|
| GET | `/health` | Liveness + Groq config status |
| POST | `/chat` | Send user message; returns assistant reply + proposals |
| GET | `/today` | Dashboard payload |
| GET | `/calendar/sessions` | All sessions |
| POST | `/calendar/sessions` | Create manual session |
| DELETE | `/calendar/sessions/{id}` | Delete session |
| POST | `/calendar/proposals/decide` | Approve/reject proposal batch |
| POST | `/calendar/sessions/outcome` | Mark completed/missed |
| GET/POST/PATCH/DELETE | `/categories` | Category CRUD |
| POST | `/categories/reclassify` | Re-match categories on existing sessions |
| GET/DELETE | `/goals`, `/goals/{id}` | Goal list + archive/remove |
| GET/POST/… | `/conversations/...` | Conversation management |
| GET | `/ai/usage` | Groq rate-limit headers summary |
| GET/POST | `/sparks`, `/sparks/{id}/promote` | Sparks capture |

Full client wrappers live in `frontend/src/api.ts`.

---

## Build commands

### Frontend production build

```bash
cd frontend
npm run build        # tsc -b && vite build → frontend/dist/
npm run preview      # serve dist locally on :4173 (no /api proxy unless you configure it)
```

**Production preview with a local backend:**

```bash
VITE_API_BASE=http://127.0.0.1:8787 npm run build
npm run preview
```

### Backend tests

```bash
cd backend
source .venv/bin/activate
PYTHONPATH=. pytest -q
```

Tests mock Groq where needed (`backend/tests/fake_ai.py`). One weekly-plan test may fail on sample counts — check output if `pytest` is not fully green.

### Typecheck only (frontend)

```bash
cd frontend
npx tsc -b --noEmit
```

---

## Debugging guide

### Chat says Cloud AI unavailable

1. Backend running? `curl http://127.0.0.1:8787/health`
2. `GROQ_API_KEY` set in `backend/.env`?
3. `BUDDY_AI_ENABLED=1`?
4. Check backend terminal for Groq HTTP errors (model ID, rate limit, auth)
5. Settings page → health panel should show Groq status

### Frontend cannot reach backend (network tab)

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `404` on `/api/health` in dev | Backend not running | Start uvicorn on `:8787` |
| CORS error in browser | Origin not allowed | Add origin to `BUDDY_CORS_ORIGINS` on backend |
| `Failed to fetch` in production | Wrong `VITE_API_BASE` | Rebuild frontend with correct URL |
| Mixed content | HTTPS site calling HTTP API | Backend must be HTTPS too |

### Calendar empty after approving proposals

1. Confirm approve clicked once (second approve should not duplicate)
2. `GET /calendar/sessions` in curl or Network tab
3. Check session `status` is `scheduled`, not still `proposed`

### Database inspection

```bash
sqlite3 backend/data/buddy.db ".tables"
sqlite3 backend/data/buddy.db "SELECT id, title, status FROM goals LIMIT 5;"
sqlite3 backend/data/buddy.db "SELECT id, title, start_at, status FROM sessions ORDER BY start_at LIMIT 10;"
```

Schema migrations run automatically on startup (`backend/app/migrations.py`).

### localStorage keys (frontend)

| Key | Purpose |
|-----|---------|
| `buddy.conversationId` | Active Chat conversation |
| `buddy.editGoalHint` | Goals → Chat “edit this goal” handoff |

Clear these in DevTools → Application → Local Storage if Chat behaves oddly.

---

## Deploying as a web app

Buddy is **two services**: static frontend + long-running backend with persistent SQLite.

### Why not Vercel-only?

- Backend is FastAPI + **SQLite on disk**
- Vercel serverless functions have **ephemeral filesystem** — the DB would reset
- Chat needs a always-on process, not cold-start functions

**Working pattern:** Vercel (frontend) + Railway / Render / Fly (backend + volume).

### A. Backend (Railway or Render example)

1. New service, repo root **`backend/`**
2. Start command:

   ```bash
   uvicorn app.main:app --host 0.0.0.0 --port $PORT
   ```

3. Environment:

   ```bash
   GROQ_API_KEY=...
   GROQ_MODEL=openai/gpt-oss-120b
   BUDDY_AI_ENABLED=1
   BUDDY_HOST=0.0.0.0
   BUDDY_DB_PATH=/data/buddy.db
   BUDDY_CORS_ORIGINS=https://your-app.vercel.app,http://localhost:5173
   ```

4. **Persistent volume** mounted at `/data` (required — without it, data is lost on redeploy)

5. Note the public URL, e.g. `https://buddy-api.up.railway.app`

### B. Frontend (Vercel)

1. Import GitHub repo
2. **Root Directory:** `frontend`
3. **Build Command:** `npm run build`
4. **Output Directory:** `dist`
5. **Environment variable (Production):**

   ```bash
   VITE_API_BASE=https://buddy-api.up.railway.app
   ```

6. `frontend/vercel.json` handles SPA routing for React Router

7. Redeploy after changing `VITE_API_BASE` (it is baked in at build time)

### C. Verify production

- Open `https://your-app.vercel.app/settings` → health should be OK
- Chat with a test goal
- Calendar shows approved sessions

### Fast solo hack (today, not durable)

Run backend locally, expose with **ngrok** or **cloudflared**, set `VITE_API_BASE` to the tunnel URL, deploy frontend only. Works for demos; tunnel URL changes when restarted.

---

## macOS desktop app (optional)

`src-tauri/` packages the same frontend against a bundled local backend. See Tauri docs. Packaged builds should set `BUDDY_DB_PATH` under Application Support — never inside the signed `.app` bundle.

Buddy v1 data may still exist at:

`~/Library/Application Support/com.liamgk.buddy/`

---

## Branches and pull requests

Active product work lives on **`codex/conversational-planning-ui`**.

Stacked review PRs (merge in order **#3 → #8**):

| PR | Title |
|----|-------|
| [#3](https://github.com/Liam-Key24/Buddy/pull/3) | Conversational planning + Groq control plane |
| [#4](https://github.com/Liam-Key24/Buddy/pull/4) | Calendar categories + manual events |
| [#5](https://github.com/Liam-Key24/Buddy/pull/5) | Goals page |
| [#6](https://github.com/Liam-Key24/Buddy/pull/6) | Settings page |
| [#7](https://github.com/Liam-Key24/Buddy/pull/7) | Brand icon |
| [#8](https://github.com/Liam-Key24/Buddy/pull/8) | Shell polish (Today, Chat, Sparks) |

**Fast path:** merge `codex/conversational-planning-ui` or `pr/06-shell-polish` directly to `main` — same code as the full stack.

---

## Manual acceptance

Follow **`docs/ACCEPTANCE.md`** with a real or mocked Groq key. Core journey:

1. Chat → state a climbing goal → answer one clarification
2. Agree to calendar → approve proposal card
3. Sessions appear on Calendar and Today
4. Mark a session completed/missed
5. Sparks → promote to Chat (does not auto-schedule)

---

## Known gaps (not blockers for basic use)

| Area | Status |
|------|--------|
| Message edit / resend in Chat UI | Schema exists; UI not wired |
| NDJSON streaming | Not implemented; request/response only |
| Weekly plan test | May fail (`test_weekly_plan_proposes_titled_sessions_through_deadline`) |
| Mac app smoke test | Packaging exists; verify locally after changes |

---

## Security notes

- Never commit `backend/.env` or Groq keys
- Groq key stays on the backend only
- Set ZDR on the Groq key for production use
- Restrict `BUDDY_CORS_ORIGINS` to your real frontend domains in production

---

## Quick reference card

```bash
# Terminal 1 — backend
cd backend && source .venv/bin/activate && set -a && source .env && set +a
PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787 --reload

# Terminal 2 — frontend
cd frontend && npm run dev

# Tests
cd backend && PYTHONPATH=. pytest -q

# Production frontend build
cd frontend && VITE_API_BASE=https://your-api.example.com npm run build
```

Local Qwen/MLX weights under `brain/models/` have been removed. Buddy uses Cloud AI (Groq) only.
