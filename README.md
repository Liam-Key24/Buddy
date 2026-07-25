# Buddy

A lightweight, local-first AI desktop assistant for macOS.

Buddy is a **thin orchestrator**. Capabilities live in plugins; intelligence
lives in the Brain; persistence lives in Memory.

```text
                UI
                 |
              Buddy
           (route only)
                 |
    --------------------------------
    |              |               |
 Memory          Brain            Core
    |                              |
Intelligence                   PluginManager
 (internal)            Filesystem · Git · Calendar · Terminal · Coder · Notes
```

Clarification validates plans before Core. Personality only styles presentation.

## Turn flow

```text
User → Buddy → Memory.get_context (soft-fail)
            → Brain.plan
            → Clarification (pending state in Memory)
                 ├── missing → Personality question → UI
                 └── ready → Core TaskRunner → Plugin
            → Memory events → Personality style → UI
```

One execution pipeline: every capability is a Core tool (`coder.run`,
`memory.handover`, `calendar.*`, …). The Code Agent page also runs through
`coder.run` — there is no separate execution world.

Buddy injects only generic session context (`conversation_id`, `workspace_path`,
`user_message`). Tools decide which fields they need.

## Responsibilities

| System | Owns |
|---|---|
| **Buddy** | Routing only (`orchestrator.rs`) |
| **Brain** | Intent, plans, tool args, replies, heuristics |
| **Clarification** | Schema gaps, Memory fills, ask facts |
| **Personality** | Tone / phrasing / emoji policy (never truncates meaning) |
| **Core** | Registry, TaskRunner, session merge, panic isolation |
| **PluginManager** | Discovery, registration, catalog, schemas |
| **Memory** | Context, pending tasks, events, handover (`MemoryApi`) |

## Adding a plugin

1. Implement `Tool` (+ optional `ToolSchema` fields) under `plugins/`.
2. Register in `all_builtin_plugins` (or `PluginManager::register_extra` for shell tools).
3. Do **not** edit Buddy’s orchestrator or AppState for domain logic.

## Prerequisites

- macOS (Apple Silicon recommended)
- Rust, Node 18+, Python 3.10+

## Setup

```bash
cd app && npm install
python3 -m venv brain/venv && source brain/venv/bin/activate
pip install -r brain/requirements.txt
```

Keep the repo on disk (Brain + MLX run from `brain/venv`).

## Run (dev)

```bash
cd app && npm run tauri dev
```

With **Auto-start MLX** enabled (default), Buddy starts MLX (`:8001`) and Brain
(`:8002`) on launch. No separate terminal is required.

To start MLX manually instead:

```bash
./brain/scripts/start_mlx.sh   # optional if auto-start is off
cd app && npm run tauri dev
```

## Desktop build (out of dev)

```bash
cd app && npm run tauri build
```

The app is at:

`app/src-tauri/target/release/bundle/macos/Buddy.app`

### First launch tips

1. Prefer launching once from the repo tree (or set `BUDDY_PROJECT_ROOT` to this
   repo) so Buddy can find `brain/venv` and persist `project_root` in settings.
2. Copy `Buddy.app` to Applications / Dock after that — later launches reuse the
   saved path (also tries `~/Desktop/BUDDY`, etc.).
3. Enable **Settings → Auto-start MLX** (on by default for new installs).
4. First MLX boot may download/load the model and take several minutes; watch
   the MLX status indicator in the sidebar.

Override the repo path any time:

```bash
export BUDDY_PROJECT_ROOT=/Users/you/Desktop/BUDDY
open /path/to/Buddy.app
```

Or after a release build:

```bash
./scripts/open-buddy.sh
```

## Testing

```bash
cargo test -p buddy-core -p buddy-clarification -p buddy-personality -p buddy-plugins
cargo test -p buddy-calendar
cargo test -p buddy-app --lib
cd app && npm test
```
