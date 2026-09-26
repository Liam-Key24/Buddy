# Buddy — single-truth production architecture

## Product truth

Buddy turns unclear goals into realistic calendar actions, tracks what happens, and proposes adaptations while the user retains final control.

```text
Natural language
→ one AI interpretation
→ canonical validated actions
→ proposed plan
→ user approval
→ calendar
→ outcome tracking
→ proposed adaptation
```

## Production topology

```text
Mac browser / phone browser / future thin client
                ↓
          private Tailscale only
                ↓
     one private HTTPS origin (Tailscale Serve → localhost)
                ↓
         FastAPI control plane (bind 127.0.0.1)
          ↙              ↘
 one SQLite database      Groq (ambiguity only)
```

## Rules

1. The **server** owns the only production backend and the only production database.
2. Mac and phone are **clients** of that server (same origin: UI + `/api`).
3. Local development may use a disposable DB under `backend/data/`; it must never silently act as production.
4. After server migration, Tauri must not launch a separate production backend. Keep Tauri in-tree as a future thin client only.
5. Do not maintain indefinite local/server compatibility shims.
6. Do not expose the API on a public interface or public Oracle firewall port.
7. Do not open a second database, agent framework, planner, or router.
8. Calendar mutations require user approval; adaptations are proposals only.
9. Normal chat: **one** Groq request; at most **one** recovery attempt.
10. Secrets live outside Git (`/etc/buddy/env`). Production DB lives outside the Git checkout (`/var/lib/buddy/`).

## Canonical paths (Oracle host)

| Role | Path |
|------|------|
| App releases | `/opt/buddy/releases/<id>/` |
| Current release symlink | `/opt/buddy/current` |
| Data + SQLite | `/var/lib/buddy/buddy.db` |
| Backups | `/var/lib/buddy/backups/` |
| Env / secrets | `/etc/buddy/env` (mode `0600`) |
| Service user | `buddy` (non-root) |
| Process bind | `127.0.0.1:8787` |
| Edge | Tailscale Serve → `http://127.0.0.1:8787` |

## Accounts

Two private accounts (Liam + partner). No public registration. Personal data is owner-scoped (`owner_user_id` on conversations, goals, sessions, sparks, and related tables). Shared calendars are deferred.

Bootstrap on first start (no UI register) from `/etc/buddy/env`:

```text
BUDDY_USER_1=liam:password
BUDDY_USER_2=partner:password
BUDDY_MIGRATE_OWNER=liam
BUDDY_SESSION_SECRET=<long random>
BUDDY_COOKIE_SECURE=1
```

Existing single-tenant rows are assigned to `BUDDY_MIGRATE_OWNER`. Auth is an HttpOnly `buddy_session` cookie (SameSite=Lax, Path=/, Secure behind Tailscale Serve HTTPS). Bind remains `127.0.0.1:8787`; Serve is the only edge. Do not change the Oracle firewall / VCN / public ports.

## Out of scope for the production release path

- Vercel + Railway/Render as the intended architecture (legacy README pattern)
- Uploading `.venv`, `node_modules`, model weights, or the full Mac working tree
- Modifying Oracle billing, VCN firewall, or public SSH
