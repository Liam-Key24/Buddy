#!/usr/bin/env bash
# First-time host preparation for Buddy (idempotent). Does not deploy code or start traffic.
# Never modifies Oracle billing, VCN, or public firewall.
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root (sudo)" >&2
  exit 1
fi

echo "==> verifying Tailscale is present"
if ! command -v tailscale >/dev/null 2>&1; then
  echo "tailscale not installed; install/connect before Buddy" >&2
  exit 1
fi
if ! tailscale status >/dev/null 2>&1; then
  echo "tailscale status failed; connect the node first" >&2
  exit 1
fi
echo "tailscale ok: $(tailscale ip -4 2>/dev/null || echo unknown)"

id -u buddy >/dev/null 2>&1 || useradd --system --home /var/lib/buddy --shell /usr/sbin/nologin buddy

install -d -o buddy -g buddy -m 0750 /var/lib/buddy
install -d -o buddy -g buddy -m 0750 /var/lib/buddy/backups
install -d -o root -g root -m 0755 /opt/buddy
install -d -o root -g root -m 0755 /opt/buddy/releases
install -d -o root -g buddy -m 0750 /etc/buddy

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [[ ! -f /etc/buddy/env ]]; then
  install -o root -g buddy -m 0640 "${SCRIPT_DIR}/env.production.example" /etc/buddy/env
  echo "wrote /etc/buddy/env from example — edit secrets before start"
else
  echo "/etc/buddy/env already exists — left unchanged"
fi

install -m 0644 "${SCRIPT_DIR}/systemd/buddy.service" /etc/systemd/system/buddy.service
install -m 0644 "${SCRIPT_DIR}/systemd/buddy-backup.service" /etc/systemd/system/buddy-backup.service
install -m 0644 "${SCRIPT_DIR}/systemd/buddy-backup.timer" /etc/systemd/system/buddy-backup.timer
systemctl daemon-reload
systemctl enable buddy.service buddy-backup.timer

echo "install scaffolding complete"
echo "next: edit /etc/buddy/env, then run update.sh with a release archive (when approved)"
