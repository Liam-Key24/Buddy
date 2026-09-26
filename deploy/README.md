# Buddy private server operations

**Do not run these scripts against the Oracle host until Liam explicitly approves deployment.**

Topology: see [`docs/SINGLE_TRUTH.md`](../docs/SINGLE_TRUTH.md).

## Prerequisites (already done on buddy-server)

- Ubuntu OCI instance with Tailscale connected as `buddy-server-vnic`
- Public SSH blocked; private SSH via Tailscale `100.101.7.38`
- Persistent firewall rules; Tailscale starts on boot

These scripts **never** change Oracle billing, VCN, or public firewall.

## Layout

| Path | Purpose |
|------|---------|
| `/opt/buddy/releases/<id>` | Immutable release tree |
| `/opt/buddy/current` | Symlink to active release |
| `/var/lib/buddy/buddy.db` | Production SQLite |
| `/var/lib/buddy/backups/` | Timestamped backups |
| `/etc/buddy/env` | Secrets (mode 0600) |
| systemd `buddy.service` | Uvicorn, one worker, `127.0.0.1:8787` |
| systemd `buddy-backup.timer` | Daily online backup |

## Scripts

| Script | Role |
|--------|------|
| `install.sh` | First-time user, dirs, env template, systemd units (idempotent) |
| `update.sh` | Backup DB → unpack release → migrate → flip symlink → restart → health |
| `rollback.sh` | Point `current` at previous release and restart |
| `backup_buddy.py` | SQLite online backup + integrity check |
| `restore_buddy.py` | Restore into a temp path and verify (or replace with `--apply`) |

## First install (when approved)

1. From a Mac with Tailscale + unlocked `~/.ssh/buddy_oracle`:

   ```bash
   ssh ubuntu@100.101.7.38  # or opc@ — match the image user
   ```

2. Copy a **clean release archive** (tracked source + prebuilt `frontend/dist`), not the 779 MB working tree.

3. On the host:

   ```bash
   sudo ./deploy/install.sh
   sudo cp /etc/buddy/env.example /etc/buddy/env   # if install left a template
   sudoedit /etc/buddy/env   # GROQ_API_KEY, BUDDY_USER_1/2, BUDDY_MIGRATE_OWNER, BUDDY_SESSION_SECRET, BUDDY_COOKIE_SECURE=1
   sudo ./deploy/update.sh /path/to/buddy-release.tar.gz
   ```

4. Edge HTTPS (Tailscale Serve — no public port):

   ```bash
   sudo tailscale serve --bg 8787
   # or HTTPS: sudo tailscale serve --bg https / http://127.0.0.1:8787
   ```

5. Verify:

   ```bash
   curl -sS http://127.0.0.1:8787/api/health
   # from Mac via Tailscale Serve URL / MagicDNS
   ```

## Rollback

```bash
sudo ./deploy/rollback.sh
```

Requires at least one prior release under `/opt/buddy/releases/`.

## Non-goals

- Do not bind `0.0.0.0` for Buddy.
- Do not install MLX / local LLM stacks.
- Do not commit `/etc/buddy/env` or database files.
