# Server readiness baseline

Recorded on branch `cursor/server-readiness-single-truth` at commit after branch creation from `main` (`f0e4383`).

## Architecture today

```text
Browser / Vite (dev) or Tauri
        │  /api/*  (Vite rewrites → backend root paths)
        ▼
FastAPI ControlPlane (:8787, 127.0.0.1)
   ├── SQLite buddy.db
   └── Groq (Cloud AI) for structured BuddyTurn only
```

- One product loop: NL → one AI interpretation → validated ops → proposal → approval → calendar → outcomes.
- No authentication; all rows are global.
- Tauri can spawn a second local backend + DB (Application Support).

## Test results (Phase 0)

| Check | Result |
|-------|--------|
| Backend `pytest` | **61 passed** (at branch cut); **63 passed** after deploy-ops + `/api` health tests |
| Frontend `tsc -b && vite build` | **passed** |
| Frontend Vitest | **12 passed** |
| Live Groq call | **not run** (forbidden in Phase 0) |

## Database

| Item | Value |
|------|-------|
| Dev path | `backend/data/buddy.db` |
| Schema version | **8** |
| Integrity | **ok** (SQLite online backup API) |
| Backup | `~/BuddyBackups/buddy-20260926T134927Z.db` (outside repo) |

Tables: `ai_requests`, `approval_events`, `buddy_meta`, `categories`, `chat_folders`, `conversations`, `fixed_blocks`, `goals`, `learned_preferences`, `messages`, `sessions`, `sparks`, `turns`.

## Repository size

| Scope | Size |
|-------|------|
| Tracked source (`git ls-files`) | ~4.6 MB |
| Working tree (`du -sh`) | ~779 MB |

Bloat is local artefacts (`.venv`, `node_modules`, caches, sidecar binaries, models). These must never be uploaded.

## Known gaps vs production plan

1. No auth / ownership (`owner_user_id`).
2. `/health` exposes DB path and host.
3. API routes are root-mounted; Vite strips `/api` in dev only.
4. No single-origin static+API production serve path.
5. README still documents Vercel/Railway as the deploy pattern.
6. No systemd / backup / restore / install scripts for Oracle+Tailscale.
7. Adaptation reconciler, Spark resurfacing schedule, bounded AI attempts — product phases still open.
8. Frontend has some Vitest coverage; login/auth tests not yet applicable.

## Never deploy

- `.venv/`, `node_modules/`, `frontend/dist/` built on Mac for upload as “source”
- `.env`, `*.db`, `*.db-wal`, `*.db-shm`
- `brain/models/**` weights, MLX / Qwen artefacts
- `.tmp-logo/`, `frontend/.vite/`, `src-tauri/target/`, `src-tauri/sidecars/*`
- Logs, Keychain material, Groq keys
- The full ~779 MB working tree

Deploy only a clean release archive of tracked source (or CI-built frontend assets) plus secrets placed outside Git on the host.

## Progress on this branch

Completed locally (not deployed):

- `docs/SINGLE_TRUTH.md` — production topology
- `deploy/` — install/update/rollback/backup/restore/systemd/package_release
- Canonical `/api/*` routes + sanitised `/api/health`
- Optional `BUDDY_STATIC_DIR` SPA serve from FastAPI
- Vite proxy keeps `/api` prefix (no rewrite)

Still open before definition-of-done:

1. Auth + `owner_user_id` scoping (two accounts)
2. Bounded Groq attempts + truthful async Stop
3. PlanReconciler adaptations
4. Spark resurfacing schedule
5. Explicit approval to upload/run `install.sh` / `update.sh` on Oracle
