#!/usr/bin/env bash
# Wait for local import → validate → pg_dump → upload to VPS.
#
# One-shot (default):
#   VPS_HOST=72.60.147.2 VPS_USER=root ./scripts/pc_to_vps_sync.sh
#
# Flags:
#   --wait-only     Monitor import until run_full_import.sh exits
#   --upload-only   Skip wait; dump + upload (DB must be ready)
#   --dump-only     Dump + checksum only (no upload)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# Re-exec under docker group when socket permission denied (common after newgrp in another terminal).
if ! docker info >/dev/null 2>&1; then
  if groups | grep -qw docker; then
    exec sg docker -c "cd '$ROOT' && bash '$0' $(printf '%q ' "$@")"
  fi
  echo "ERROR: docker not available — run: newgrp docker" >&2
  exit 1
fi

# --- Defaults (override via env) ---
VPS_HOST="${VPS_HOST:-72.60.147.2}"
VPS_USER="${VPS_USER:-root}"
DUMP_DIR="${DUMP_DIR:-$HOME/opencnpj-dumps}"
DUMP_TAG="${DUMP_TAG:-$(date +%Y%m)}"
REMOTE_INCOMING="${REMOTE_INCOMING:-/var/lib/opencnpj/incoming}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"
MIN_ESTABELECIMENTOS="${MIN_ESTABELECIMENTOS:-20000000}"
WAIT_POLL_SEC="${WAIT_POLL_SEC:-30}"
LOG_FILE="${LOG_FILE:-/tmp/pc_to_vps_sync.log}"

MODE="full"
for arg in "$@"; do
  case "$arg" in
    --wait-only) MODE="wait" ;;
    --upload-only) MODE="upload" ;;
    --dump-only) MODE="dump" ;;
    -h|--help)
      sed -n '2,20p' "$0"
      exit 0
      ;;
    *) echo "Unknown flag: $arg" >&2; exit 1 ;;
  esac
done

log() {
  local line="[$(date '+%Y-%m-%d %H:%M:%S')] $*"
  echo "$line" | tee -a "$LOG_FILE"
}

psql_docker() {
  docker exec "$PG_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" "$@"
}

row_counts() {
  psql_docker -t -A -c "
    SELECT 'empresas=' || COUNT(*) FROM empresas
    UNION ALL SELECT 'estabelecimentos=' || COUNT(*) FROM estabelecimentos
    UNION ALL SELECT 'socios=' || COUNT(*) FROM socios
    UNION ALL SELECT 'simples=' || COUNT(*) FROM simples;
  " 2>/dev/null | paste -sd' ' -
}

estab_count() {
  psql_docker -t -A -c "SELECT COUNT(*) FROM estabelecimentos" 2>/dev/null | tr -d '[:space:]'
}

import_running() {
  pgrep -f "run_full_import\.sh" >/dev/null 2>&1 \
    || pgrep -f "cmd/importer" >/dev/null 2>&1
}

wait_for_import() {
  log "Waiting for import (poll every ${WAIT_POLL_SEC}s) — tail: $LOG_FILE"
  log "Import progress also in: /tmp/import_progress.log"

  while import_running; do
    local counts estab
    counts="$(row_counts 2>/dev/null || echo "db=unavailable")"
    estab="$(estab_count 2>/dev/null || echo 0)"
    log "import running | $counts | estabelecimentos=$estab"
    sleep "$WAIT_POLL_SEC"
  done

  log "Import process finished — validating row counts"
  local estab
  estab="$(estab_count)"
  if [[ -z "$estab" || "$estab" -lt "$MIN_ESTABELECIMENTOS" ]]; then
    log "ERROR: estabelecimentos=$estab (expected >= $MIN_ESTABELECIMENTOS)" >&2
    exit 1
  fi
  log "OK: estabelecimentos=$estab"
  row_counts | while read -r line; do log "  $line"; done
}

apply_migrations() {
  log "Applying CNPJ migrations on localhost:5434"
  DATABASE_URL="postgres://${DB_USER}:receita_password@localhost:5434/${DB_NAME}?sslmode=disable" \
    go run ./cmd/migrate up
}

dump_paths() {
  local dump="$DUMP_DIR/opencnpj_cnpj_${DUMP_TAG}.dump"
  printf '%s\n%s\n%s\n' "${dump}.zst" "${dump}.zst.sha256" "$dump"
}

local_dump() {
  mkdir -p "$DUMP_DIR"
  local -a paths
  mapfile -t paths < <(dump_paths)
  local archive="${paths[0]}"
  local checksum="${paths[1]}"
  local dump="${paths[2]}"

  if [[ -f "$archive" && "${FORCE_DUMP:-0}" != "1" ]]; then
    log "Dump archive exists: $archive (set FORCE_DUMP=1 to recreate)"
    return 0
  fi

  log "Validating database before pg_dump"
  psql_docker -c "SELECT count(*) AS estabelecimentos FROM estabelecimentos"
  psql_docker -c "SELECT pg_get_partkeydef('estabelecimentos'::regclass)"

  log "pg_dump started → $dump (this may take 30–90 min)"
  local start
  start=$(date +%s)
  docker exec "$PG_CONTAINER" pg_dump -Fc --no-owner --no-acl -U "$DB_USER" -d "$DB_NAME" >"$dump"
  log "pg_dump done in $(( $(date +%s) - start ))s — compressing with zstd"
  zstd -T0 -19 -f "$dump"
  sha256sum "$archive" | tee "$checksum"
  local size
  size=$(du -h "$archive" | cut -f1)
  log "Dump ready: $archive ($size)"
}

upload_to_vps() {
  local -a paths
  mapfile -t paths < <(dump_paths)
  local archive="${paths[0]}"
  local checksum="${paths[1]}"

  if [[ ! -f "$archive" ]]; then
    log "ERROR: missing dump $archive — run without --upload-only first" >&2
    exit 1
  fi

  log "Preparing VPS incoming dir: ${VPS_USER}@${VPS_HOST}:${REMOTE_INCOMING}"
  ssh -o BatchMode=yes "${VPS_USER}@${VPS_HOST}" "mkdir -p '${REMOTE_INCOMING}'"

  log "Uploading to VPS (rsync — may take a while)"
  rsync -avP --partial "$archive" "$checksum" \
    "${VPS_USER}@${VPS_HOST}:${REMOTE_INCOMING}/"

  log "Verifying checksum on VPS"
  ssh -o BatchMode=yes "${VPS_USER}@${VPS_HOST}" \
    "cd '${REMOTE_INCOMING}' && sha256sum -c '$(basename "$checksum")'"

  log "Upload complete: ${VPS_USER}@${VPS_HOST}:${REMOTE_INCOMING}/$(basename "$archive")"
  log "Next on VPS: bootstrap Postgres + ./deploy/saas/monthly-cnpj-sync.example.sh vps-restore"
}

main() {
  : >"$LOG_FILE"
  log "pc_to_vps_sync mode=$MODE host=${VPS_USER}@${VPS_HOST} tag=$DUMP_TAG"

  case "$MODE" in
    wait)
      wait_for_import
      ;;
    dump)
      wait_for_import
      apply_migrations || log "WARN: migrations step failed (continuing)"
      local_dump
      ;;
    upload)
      local_dump
      upload_to_vps
      ;;
    full)
      wait_for_import
      apply_migrations || log "WARN: migrations step failed (continuing)"
      local_dump
      upload_to_vps
      ;;
  esac

  log "Done."
}

main
