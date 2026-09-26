#!/usr/bin/env bash
# Deploy a clean Buddy release archive. Backs up DB first; aborts on backup/migrate/health failure.
# Usage: sudo ./deploy/update.sh /path/to/buddy-release.tar.gz
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root (sudo)" >&2
  exit 1
fi

ARCHIVE="${1:-}"
if [[ -z "${ARCHIVE}" || ! -f "${ARCHIVE}" ]]; then
  echo "usage: $0 /path/to/buddy-release.tar.gz" >&2
  exit 1
fi

# shellcheck disable=SC1091
set -a
source /etc/buddy/env
set +a

DB_PATH="${BUDDY_DB_PATH:-/var/lib/buddy/buddy.db}"
BACKUP_DIR="${BUDDY_BACKUP_DIR:-/var/lib/buddy/backups}"
RELEASE_ID="$(date -u +%Y%m%dT%H%M%SZ)"
RELEASE_DIR="/opt/buddy/releases/${RELEASE_ID}"
CURRENT="/opt/buddy/current"
PREVIOUS=""

if [[ -L "${CURRENT}" ]]; then
  PREVIOUS="$(readlink -f "${CURRENT}" || true)"
fi

echo "==> pre-update backup"
if [[ -f "${DB_PATH}" ]]; then
  if [[ -x "${CURRENT}/backend/.venv/bin/python" ]]; then
    "${CURRENT}/backend/.venv/bin/python" "${CURRENT}/deploy/backup_buddy.py" --db "${DB_PATH}" --dir "${BACKUP_DIR}"
  else
    python3 "${BASH_SOURCE[0]%/*}/backup_buddy.py" --db "${DB_PATH}" --dir "${BACKUP_DIR}"
  fi
else
  echo "no existing database at ${DB_PATH} — first boot will create one"
fi

echo "==> unpack ${ARCHIVE} → ${RELEASE_DIR}"
mkdir -p "${RELEASE_DIR}"
tar -xzf "${ARCHIVE}" -C "${RELEASE_DIR}" --strip-components=1

if [[ ! -f "${RELEASE_DIR}/backend/app/main.py" ]]; then
  echo "release missing backend/app/main.py — refusing" >&2
  rm -rf "${RELEASE_DIR}"
  exit 1
fi
if [[ ! -f "${RELEASE_DIR}/frontend/dist/index.html" ]]; then
  echo "release missing frontend/dist/index.html — build UI before packaging" >&2
  rm -rf "${RELEASE_DIR}"
  exit 1
fi

echo "==> python venv + deps"
python3 -m venv "${RELEASE_DIR}/backend/.venv"
"${RELEASE_DIR}/backend/.venv/bin/pip" install --upgrade pip
"${RELEASE_DIR}/backend/.venv/bin/pip" install -r "${RELEASE_DIR}/backend/requirements.txt"

echo "==> migration smoke (import + init_db)"
(
  cd "${RELEASE_DIR}/backend"
  export BUDDY_DB_PATH="${DB_PATH}"
  export BUDDY_STATIC_DIR="${RELEASE_DIR}/frontend/dist"
  export PYTHONPATH=.
  .venv/bin/python - <<'PY'
from app.db import get_connection, init_db
from app.config import load_settings
s = load_settings()
conn = get_connection(s.db_path)
init_db(conn)
conn.close()
print("migrations ok")
PY
)

echo "==> flip current symlink"
ln -sfn "${RELEASE_DIR}" "${CURRENT}.new"
mv -Tf "${CURRENT}.new" "${CURRENT}"

chown -R buddy:buddy /var/lib/buddy
# Release tree readable by service user
chown -R root:buddy "${RELEASE_DIR}"
chmod -R g+rX "${RELEASE_DIR}"

echo "==> restart buddy"
systemctl restart buddy.service
sleep 1
systemctl is-active --quiet buddy.service

echo "==> health"
HEALTH="$(curl -fsS "http://127.0.0.1:${BUDDY_PORT:-8787}/api/health" || true)"
echo "${HEALTH}"
echo "${HEALTH}" | grep -q '"ok": true' || {
  echo "health check failed — attempting rollback to previous release" >&2
  if [[ -n "${PREVIOUS}" && -d "${PREVIOUS}" ]]; then
    ln -sfn "${PREVIOUS}" "${CURRENT}"
    systemctl restart buddy.service || true
  fi
  exit 1
}

echo "update ok release=${RELEASE_ID}"
if [[ -n "${PREVIOUS}" ]]; then
  echo "previous=${PREVIOUS}"
fi
