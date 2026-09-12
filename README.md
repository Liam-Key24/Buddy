# Buddy

A lightweight, local-first AI desktop assistant for macOS.

Chat is the mouth. A local model (MLX) decides when to use tools. Rust runs them.
Calendar is the deepest capability: **look**, **pin**, **organize**.

```text
                UI  (Chat · Calendar · Spark · Code · Settings)
                 |
              Buddy  (route + native tool loop)
                 |
    --------------------------------
    |              |               |
 Memory          Brain            Core
                   |               |
              /v1/complete    Plugin tools
                              look · pin · organize
                              fs · spark · coder · …
```

## Turn flow

Composer is **one Buddy**. Canonical syntax and plugin extract skip the model.
Messy language, dumps, and multi-intent go to Qwen. Llama may answer short chitchat
only when Qwen is not already loaded — the user never picks a model.

```text
User → Memory context (soft-fail)
     → confirm/cancel open proposal
     → canonical syntax / plugin extract → clarify → Core → phrase
     → else Qwen native loop (max 8 steps, life tools attached)
           ├── tool_calls → clarify → Core → continue
           ├── ask / propose → wait for radios or “yes”
           └── text reply → Personality → UI
     → matching workspace opens (doc, week, fitness, …)
```

Organize always **proposes** first (ghost week). Confirm with `yes` / Accept.
Clarification asks only for missing required fields. Personality only styles tone.

## Calendar

| Kind | Examples | Tool |
|---|---|---|
| **Look** | What’s on today? When am I free tomorrow? | `calendar.look` |
| **Pin** | Dentist tomorrow at 2pm | `calendar.pin` (fixed clock) |
| **Organize** | Book climbing this week; plan my week… | `calendar.organize` (flexible) |

Work / Sleep lifestyle blocks are protected. The organizer schedules around them.

Parser goldens: `cd brain && ./venv/bin/python test_parser_calendar.py`  
Hold-out paraphrases: `cd brain && ./venv/bin/python evals/run_holdout.py`

## Responsibilities

| System | Owns |
|---|---|
| **Buddy** | One front door: canonical route → clarify → Core; else Qwen (`orchestrator.rs`, `native_loop.rs`) |
| **Brain** | `/v1/complete` tool calls; `/chat/talk` hidden chat stream; `/chat/plan` eval-only |
| **Clarification** | Schema gaps, radio asks |
| **Personality** | Tone / phrasing (never truncates meaning) |
| **Core** | Registry, TaskRunner |
| **Plugins** | Tools + schemas + catalog |
| **Memory** | Context, pending jobs, last-look, handover |

Adding a plugin: implement `Tool` (+ `ToolSchema`), register in `all_builtin_plugins`. Do not put domain logic in the orchestrator.

## Prerequisites

- macOS (Apple Silicon recommended)
- Rust, Node 18+, Python 3.10+

## Setup

```bash
cd app && npm install
python3 -m venv brain/venv && source brain/venv/bin/activate
pip install -r brain/requirements.txt
```

## Run (dev)

```bash
cd app && npm run tauri dev
```

Brain (`:8002`) starts on launch. MLX (`:8001`) starts when a turn needs a model
(canonical look / extract skip it) and idles off after ~90s unless **Settings → Keep
model loaded after chat** is on.

```bash
./brain/scripts/download_qwen14b.sh   # ~8 GB, resume-safe (16 GB Macs)
./brain/scripts/start_mlx.sh          # optional manual start
```

The assistant is Qwen3-14B-4bit (`brain/models/Qwen3-14B-4bit` when the shards
are complete). Llama 3.2 3B may answer short chitchat if Qwen is not resident.
30B-A3B-4bit does not fit a 16 GB Air.

## Desktop build

```bash
cd app && npm run tauri build
```

App: `target/release/bundle/macos/Buddy.app`

Set `BUDDY_PROJECT_ROOT` if you move the `.app` away from the repo.

## Testing

```bash
cargo test -p buddy-core -p buddy-clarification -p buddy-personality -p buddy-plugins
cargo test -p buddy-calendar
cargo test -p buddy-app --lib
cd app && npm test
cd brain && ./venv/bin/python test_parser_calendar.py
cd brain && ./venv/bin/python evals/run_holdout.py
```
