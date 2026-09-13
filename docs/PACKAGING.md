# Packaging Buddy (macOS)

Tauri is a thin window + process supervisor only. Product logic stays in FastAPI.

## Layout

- `frontend/` — Vite React UI
- `backend/` — FastAPI + SQLite + Groq
- `src-tauri/` — macOS wrapper
- `scripts/build_sidecar.sh` — PyInstaller sidecar for the Python backend

## Secrets

Never put `GROQ_API_KEY` in the app bundle, frontend, or Git.

```bash
security add-generic-password -a buddy -s com.liamgk.buddy.groq -w 'YOUR_KEY' -U
```

The wrapper reads that Keychain item and passes it only to the localhost backend process.

Also enable **Zero Data Retention** in the Groq console for the key.

## Database

Packaged app default:

`~/Library/Application Support/com.liamgk.buddy/buddy.db`

Development default remains `backend/data/buddy.db` unless `BUDDY_DB_PATH` is set.

## Build

```bash
# 1) Sidecar
chmod +x scripts/build_sidecar.sh
./scripts/build_sidecar.sh

# 2) Frontend + app
cd frontend && npm install && npm run build && cd ..
cd src-tauri && cargo tauri build
```

Artifacts typically land under:

`src-tauri/target/release/bundle/macos/Buddy.app`
`src-tauri/target/release/bundle/dmg/Buddy_*.dmg`

## Dev (wrapper optional)

```bash
# Terminal A
cd backend && source .venv/bin/activate
PYTHONPATH=. uvicorn app.main:app --host 127.0.0.1 --port 8787 --reload

# Terminal B
cd frontend && npm run dev
```

Or `cargo tauri dev` from `src-tauri` (starts Vite + backend via the wrapper).

## Constraints

- Backend binds `127.0.0.1` only.
- One backend child; killed on app exit.
- Logs go to Application Support `backend.log` without message content or API keys.
- Signing/notarization is environment-specific and not automated here.
