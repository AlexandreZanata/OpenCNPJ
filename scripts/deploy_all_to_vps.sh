#!/usr/bin/env bash
# ONE command from your PC: dump DB → build Linux binaries → upload → deploy on VPS.
#
# Usage:
#   cd /home/zanata-servidor/OpenCNPJ
#   newgrp docker   # if needed
#   VPS_HOST=72.60.147.2 VPS_USER=root ADMIN_EMAIL=admin@comerc.app.br ./scripts/deploy_all_to_vps.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VPS_HOST="${VPS_HOST:-72.60.147.2}"
VPS_USER="${VPS_USER:-root}"
ADMIN_EMAIL="${ADMIN_EMAIL:-admin@comerc.app.br}"
DUMP_TAG="${DUMP_TAG:-$(date +%Y%m)}"
REMOTE="${VPS_USER}@${VPS_HOST}"
STAGING="${ROOT}/.deploy-staging"

if [[ "${API_ONLY:-0}" != "1" ]] && ! docker info >/dev/null 2>&1; then
  if groups | grep -qw docker; then
    exec sg docker -c "cd '$ROOT' && ADMIN_EMAIL='$ADMIN_EMAIL' VPS_HOST='$VPS_HOST' VPS_USER='$VPS_USER' DUMP_TAG='$DUMP_TAG' bash '$0'"
  fi
  echo "ERROR: run: newgrp docker" >&2
  exit 1
fi

log() { echo "[$(date '+%H:%M:%S')] $*"; }

if [[ "${API_ONLY:-0}" == "1" ]]; then
  log "API_ONLY=1 — skipping dump; uploading binaries + migrations only"
  mkdir -p "$STAGING/bin"
  export GOOS=linux GOARCH=amd64 CGO_ENABLED=0
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-api" ./cmd/api
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-migrate" ./cmd/migrate
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-seed" ./cmd/seed-saas
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-importer" ./cmd/importer
  ssh -o BatchMode=yes "$REMOTE" "mkdir -p /opt/opencnpj/bin /opt/opencnpj/config /opt/opencnpj/deploy/saas /opt/opencnpj/migrations /opt/opencnpj/scripts"
  rsync -avP "$STAGING/bin/" "${REMOTE}:/opt/opencnpj/bin/"
  rsync -avP "$ROOT/scripts/vps_first_deploy.sh" "$ROOT/scripts/vps_create_indexes.sql" "${REMOTE}:/opt/opencnpj/scripts/"
  rsync -avP "$ROOT/migrations/" "${REMOTE}:/opt/opencnpj/migrations/"
  rsync -avP "$ROOT/config/config.saas.example.yaml" "${REMOTE}:/opt/opencnpj/config/"
  rsync -avP "$ROOT/deploy/saas/" "${REMOTE}:/opt/opencnpj/deploy/saas/"
  ssh "$REMOTE" "chmod +x /opt/opencnpj/scripts/vps_first_deploy.sh && \
    ADMIN_EMAIL='${ADMIN_EMAIL}' REPO=/opt/opencnpj API_ONLY=1 \
    bash /opt/opencnpj/scripts/vps_first_deploy.sh"
  ssh "$REMOTE" "cat /etc/opencnpj/credentials.txt 2>/dev/null" | tee "$HOME/opencnpj-credentials-${DUMP_TAG}.txt"
  chmod 600 "$HOME/opencnpj-credentials-${DUMP_TAG}.txt"
  log "Credentials saved: $HOME/opencnpj-credentials-${DUMP_TAG}.txt"
  log "Done. Test: curl -s https://api.comerc.app.br/readyz"
  exit 0
fi

log "Step 1/5 — pg_dump from local Docker Postgres"
mkdir -p "$HOME/opencnpj-dumps"
DUMP="$HOME/opencnpj-dumps/opencnpj_cnpj_${DUMP_TAG}.dump"
ARCHIVE="${DUMP}.zst"
if [[ "${RESUME:-0}" == "1" ]]; then
  log "RESUME=1 — skipping dump (using existing archive)"
elif [[ ! -f "$ARCHIVE" || "${FORCE_DUMP:-0}" == "1" ]]; then
  docker exec receita-postgres pg_dump -Fc --no-owner --no-acl -U receita_user -d receita_db >"$DUMP"
  zstd -T0 -19 -f "$DUMP"
  sha256sum "$ARCHIVE" | tee "${ARCHIVE}.sha256"
else
  log "Using existing dump: $ARCHIVE"
fi

log "Step 2/5 — cross-compile Linux binaries"
mkdir -p "$STAGING/bin"
if [[ "${RESUME:-0}" == "1" && -f "$STAGING/bin/opencnpj-api" && -f "$STAGING/bin/opencnpj-importer" && "${FINISH_RESTORE:-0}" != "1" ]]; then
  log "RESUME=1 — skipping build (using existing binaries)"
else
  export GOOS=linux GOARCH=amd64 CGO_ENABLED=0
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-api" ./cmd/api
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-migrate" ./cmd/migrate
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-seed" ./cmd/seed-saas
  go build -ldflags="-s -w" -o "$STAGING/bin/opencnpj-importer" ./cmd/importer
fi

log "Step 3/5 — upload dump + binaries + scripts to VPS"
ssh -o BatchMode=yes "$REMOTE" "mkdir -p /opt/opencnpj/bin /opt/opencnpj/config /opt/opencnpj/deploy/saas /opt/opencnpj/data/refs /opt/opencnpj/migrations /opt/opencnpj/scripts /var/lib/opencnpj/incoming"
rsync -avP "$ARCHIVE" "${ARCHIVE}.sha256" "${REMOTE}:/var/lib/opencnpj/incoming/"
rsync -avP "$STAGING/bin/" "${REMOTE}:/opt/opencnpj/bin/"
rsync -avP "$ROOT/scripts/vps_first_deploy.sh" "$ROOT/scripts/vps_create_indexes.sql" "${REMOTE}:/opt/opencnpj/scripts/"
rsync -avP "$ROOT/migrations/" "${REMOTE}:/opt/opencnpj/migrations/"
rsync -avP "$ROOT/config/config.saas.example.yaml" "${REMOTE}:/opt/opencnpj/config/"
rsync -avP "$ROOT/deploy/saas/" "${REMOTE}:/opt/opencnpj/deploy/saas/"
rsync -avP "$ROOT/data/"*NATJUCSV "$ROOT/data/"*QUALSCSV "$ROOT/data/"*PAISCSV \
  "$ROOT/data/"*MOTICSV "$ROOT/data/"*MUNICCSV "$ROOT/data/"*CNAECSV \
  "${REMOTE}:/opt/opencnpj/data/refs/" 2>/dev/null || true

log "Step 4/5 — run VPS deploy (restore + API + nginx)"
  ssh "$REMOTE" "chmod +x /opt/opencnpj/scripts/vps_first_deploy.sh && \
    ADMIN_EMAIL='${ADMIN_EMAIL}' DUMP_TAG='${DUMP_TAG}' REPO=/opt/opencnpj \
    FINISH_RESTORE='${FINISH_RESTORE:-0}' API_ONLY='${API_ONLY:-0}' \
    bash /opt/opencnpj/scripts/vps_first_deploy.sh"

log "Step 5/5 — fetch credentials"
ssh "$REMOTE" "cat /etc/opencnpj/credentials.txt 2>/dev/null" | tee "$HOME/opencnpj-credentials-${DUMP_TAG}.txt"
chmod 600 "$HOME/opencnpj-credentials-${DUMP_TAG}.txt"
log "Credentials saved: $HOME/opencnpj-credentials-${DUMP_TAG}.txt"
log "Done. Test: curl -s https://api.comerc.app.br/readyz"
