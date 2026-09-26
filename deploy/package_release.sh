#!/usr/bin/env bash
# Build a clean release archive from the current checkout (no .venv / node_modules).
# Usage: ./deploy/package_release.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT}"

echo "==> frontend production build"
(cd frontend && npm run build)

STAGING="$(mktemp -d "${TMPDIR:-/tmp}/buddy-release.XXXXXX")"
NAME="buddy-release-$(date -u +%Y%m%dT%H%M%SZ)"
DEST="${STAGING}/${NAME}"
mkdir -p "${DEST}"

echo "==> copy tracked paths + built UI"
# Core trees only — never the Mac working-tree bloat
rsync -a \
  --exclude '.venv' \
  --exclude 'node_modules' \
  --exclude '__pycache__' \
  --exclude '.pytest_cache' \
  --exclude 'data/*.db' \
  --exclude 'data/*.db-*' \
  --exclude '.env' \
  backend/ "${DEST}/backend/"
rsync -a frontend/dist/ "${DEST}/frontend/dist/"
rsync -a deploy/ "${DEST}/deploy/"
rsync -a docs/SINGLE_TRUTH.md docs/SERVER_READINESS_BASELINE.md "${DEST}/docs/" 2>/dev/null || mkdir -p "${DEST}/docs"
cp -f README.md "${DEST}/README.md" 2>/dev/null || true

OUT="${ROOT}/${NAME}.tar.gz"
tar -C "${STAGING}" -czf "${OUT}" "${NAME}"
rm -rf "${STAGING}"
echo "wrote ${OUT}"
ls -lh "${OUT}"
