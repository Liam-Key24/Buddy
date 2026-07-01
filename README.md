# Buddy

A lightweight, local-first AI desktop assistant for macOS.

## Architecture

- **UI** — React + Tauri (chat interface)
- **Core** — Rust (tool registry, task runner)
- **Brain** — Python + FastAPI (reasoning via MLX)
- **Storage** — SQLite

## Prerequisites

- macOS (Apple Silicon recommended)
- Rust (`rustup`)
- Node.js 18+
- Python 3.10+

## Setup

```bash
# Frontend dependencies
cd app && npm install

# Python Brain + MLX
python3 -m venv brain/venv
source brain/venv/bin/activate
pip install -r brain/requirements.txt
```

## Running

**Terminal 1 — MLX model server:**
```bash
chmod +x brain/scripts/start_mlx.sh
./brain/scripts/start_mlx.sh
```

**Terminal 2 — Buddy app:**
```bash
cd app
npm run tauri dev
```

The Brain service starts automatically when the app launches.

## Testing MLX

```bash
source brain/venv/bin/activate
python brain/scripts/test_mlx.py
```

## Build

```bash
cd app
npm run tauri build
```

The `.app` bundle is produced in `app/src-tauri/target/release/bundle/macos/`.
# Buddy
