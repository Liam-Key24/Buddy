#!/usr/bin/env bash
# Point /opt/buddy/current at the previous release and restart.
set -euo pipefail

if [[ "${EUID}" -ne 0 ]]; then
  echo "run as root (sudo)" >&2
  exit 1
fi

CURRENT="/opt/buddy/current"
RELEASES="/opt/buddy/releases"

if [[ ! -L "${CURRENT}" ]]; then
  echo "no current release symlink" >&2
  exit 1
fi

ACTIVE="$(readlink -f "${CURRENT}")"
# shellcheck disable=SC2012
PREV="$(ls -1dt "${RELEASES}"/* 2>/dev/null | while read -r d; do
  [[ "$(readlink -f "${d}")" == "${ACTIVE}" ]] && continue
  echo "${d}"
  break
done)"

if [[ -z "${PREV}" || ! -d "${PREV}" ]]; then
  echo "no previous release to restore" >&2
  exit 1
fi

echo "rolling back ${ACTIVE} → ${PREV}"
ln -sfn "${PREV}" "${CURRENT}"
systemctl restart buddy.service
sleep 1
curl -fsS "http://127.0.0.1:${BUDDY_PORT:-8787}/api/health"
echo
echo "rollback ok"
