#!/usr/bin/env bash
# Monthly CNPJ sync — EXAMPLE ONLY (local PC + VPS operator workflow).
#
# Copy to your workstation/VPS and edit placeholders.
# Do NOT commit real hosts, users, or passwords.
#
# See: docs/ops/MONTHLY-CNPJ-SYNC.md
# opencnpj_saas is never modified by this workflow.
set -euo pipefail

# --- Local PC settings ---
LOCAL_DATABASE_URL="${LOCAL_DATABASE_URL:-postgres://receita_user:CHANGE_ME@localhost:5434/receita_db}"
DUMP_DIR="${DUMP_DIR:-$HOME/opencnpj-dumps}"
VPS_HOST="${VPS_HOST:-YOUR_VPS_IP}"
VPS_USER="${VPS_USER:-root}"
REMOTE_INCOMING="${REMOTE_INCOMING:-/var/lib/opencnpj/incoming}"

# --- VPS settings ---
INCOMING_DIR="${INCOMING_DIR:-/var/lib/opencnpj/incoming}"
TARGET_DB="${TARGET_DB:-opencnpj_cnpj}"
STAGING_DB="${STAGING_DB:-opencnpj_cnpj_new}"
OLD_DB="${OLD_DB:-opencnpj_cnpj_old}"
BAD_DB="${BAD_DB:-opencnpj_cnpj_bad}"
GRANT_READER_SQL="${GRANT_READER_SQL:-/etc/opencnpj/grant-reader.sql}"
API_SERVICE="${API_SERVICE:-opencnpj-api}"
REDIS_PORT="${REDIS_PORT:-6381}"
RESTORE_JOBS="${RESTORE_JOBS:-4}"
PSQL="${PSQL:-sudo -u postgres psql}"
PG_RESTORE="${PG_RESTORE:-sudo -u postgres pg_restore}"
SYSTEMCTL="${SYSTEMCTL:-systemctl}"
DUMP_TAG="${DUMP_TAG:-$(date +%Y%m)}"

usage() {
  cat <<'EOF'
Usage: monthly-cnpj-sync.example.sh <command>

Local PC:
  local-dump    Validate local DB, pg_dump, compress, checksum
  upload        rsync dump + checksum to VPS incoming dir

VPS (run as root):
  vps-restore   Decompress, restore to staging DB, swap, re-grant, flush cache
  vps-rollback  Rename bad DB away and restore opencnpj_cnpj_old
  vps-drop-old  Drop opencnpj_cnpj_old after validation window

Set DRY_RUN=1 to print commands without executing destructive steps.
EOF
  exit 1
}

run_cmd() {
  if [[ "${DRY_RUN:-0}" == "1" ]]; then
    echo "[DRY_RUN] $*"
    return 0
  fi
  "$@"
}

dump_paths() {
  local dump="$DUMP_DIR/opencnpj_cnpj_${DUMP_TAG}.dump"
  echo "${dump}.zst" "${dump}.zst.sha256" "$dump"
}

local_dump() {
  mkdir -p "$DUMP_DIR"
  local -a paths
  mapfile -t paths < <(dump_paths)
  local archive="${paths[0]}"
  local dump="${paths[2]}"

  echo "=== Validate local DB ==="
  psql "$LOCAL_DATABASE_URL" -c "SELECT count(*) AS estabelecimentos FROM estabelecimentos"
  psql "$LOCAL_DATABASE_URL" -c "SELECT pg_get_partkeydef('estabelecimentos'::regclass)"

  echo "=== pg_dump ==="
  pg_dump -Fc --no-owner --no-acl -f "$dump" "$LOCAL_DATABASE_URL"
  zstd -T0 -19 -f "$dump"
  sha256sum "$archive" | tee "${archive}.sha256"
  echo "Done: $archive"
}

upload() {
  local -a paths
  mapfile -t paths < <(dump_paths)
  local archive="${paths[0]}"
  local checksum="${paths[1]}"
  if [[ ! -f "$archive" ]]; then
    echo "Missing dump: $archive (run local-dump first)" >&2
    exit 1
  fi
  rsync -avP "$archive" "$checksum" "${VPS_USER}@${VPS_HOST}:${REMOTE_INCOMING}/"
}

incoming_dump() {
  echo "${INCOMING_DIR}/opencnpj_cnpj_${DUMP_TAG}.dump"
}

incoming_archive() {
  echo "${INCOMING_DIR}/opencnpj_cnpj_${DUMP_TAG}.dump.zst"
}

verify_checksum() {
  local archive
  archive="$(incoming_archive)"
  if [[ -f "${archive}.sha256" ]]; then
    (cd "$INCOMING_DIR" && sha256sum -c "opencnpj_cnpj_${DUMP_TAG}.dump.zst.sha256")
  else
    echo "WARN: no checksum file for $archive" >&2
  fi
}

decompress_dump() {
  local archive dump
  archive="$(incoming_archive)"
  dump="$(incoming_dump)"
  if [[ ! -f "$archive" ]]; then
    echo "Missing archive: $archive" >&2
    exit 1
  fi
  verify_checksum
  if [[ ! -f "$dump" ]]; then
    run_cmd zstd -d "$archive"
  fi
}

stop_api() {
  run_cmd "$SYSTEMCTL" stop "$API_SERVICE"
}

start_api() {
  run_cmd "$SYSTEMCTL" start "$API_SERVICE"
}

terminate_target_connections() {
  run_cmd "$PSQL" -v ON_ERROR_STOP=1 <<SQL
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE datname = '$TARGET_DB' AND pid <> pg_backend_pid();
SQL
}

create_staging_db() {
  run_cmd "$PSQL" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS $STAGING_DB;"
  run_cmd "$PSQL" -v ON_ERROR_STOP=1 -c "CREATE DATABASE $STAGING_DB;"
}

restore_staging() {
  local dump
  dump="$(incoming_dump)"
  run_cmd "$PG_RESTORE" --section=pre-data --no-owner --no-acl -d "$STAGING_DB" "$dump"
  PGOPTIONS="-c session_replication_role=replica" \
    run_cmd "$PG_RESTORE" --section=data -j "$RESTORE_JOBS" --no-owner --no-acl -d "$STAGING_DB" "$dump"
  run_cmd "$PG_RESTORE" --section=post-data --no-owner --no-acl -d "$STAGING_DB" "$dump"
}

validate_staging() {
  run_cmd "$PSQL" -d "$STAGING_DB" -c "SELECT count(*) AS estabelecimentos FROM estabelecimentos"
}

swap_databases() {
  terminate_target_connections
  run_cmd "$PSQL" -v ON_ERROR_STOP=1 <<SQL
ALTER DATABASE $TARGET_DB RENAME TO $OLD_DB;
ALTER DATABASE $STAGING_DB RENAME TO $TARGET_DB;
SQL
}

regrant_reader() {
  if [[ ! -f "$GRANT_READER_SQL" ]]; then
    echo "Missing grant SQL: $GRANT_READER_SQL" >&2
    exit 1
  fi
  run_cmd "$PSQL" -d "$TARGET_DB" -f "$GRANT_READER_SQL"
}

post_restore_maintenance() {
  run_cmd "$PSQL" -d "$TARGET_DB" -c "ANALYZE;"
}

flush_cnpj_cache() {
  if command -v redis-cli >/dev/null 2>&1; then
    if [[ "${DRY_RUN:-0}" == "1" ]]; then
      echo "[DRY_RUN] redis-cli -p $REDIS_PORT --scan --pattern 'cnpj:*' | xargs redis-cli -p $REDIS_PORT DEL"
      return 0
    fi
    redis-cli -p "$REDIS_PORT" --scan --pattern 'cnpj:*' \
      | xargs -r redis-cli -p "$REDIS_PORT" DEL || true
  else
    echo "WARN: redis-cli not found — skip CNPJ cache flush" >&2
  fi
}

vps_restore() {
  decompress_dump
  stop_api
  create_staging_db
  restore_staging
  validate_staging
  swap_databases
  regrant_reader
  post_restore_maintenance
  flush_cnpj_cache
  start_api
  echo "OK: $TARGET_DB restored from opencnpj_cnpj_${DUMP_TAG}.dump"
}

vps_rollback() {
  stop_api
  terminate_target_connections
  run_cmd "$PSQL" -v ON_ERROR_STOP=1 <<SQL
ALTER DATABASE $TARGET_DB RENAME TO $BAD_DB;
ALTER DATABASE $OLD_DB RENAME TO $TARGET_DB;
SQL
  start_api
  echo "OK: rolled back to $OLD_DB → $TARGET_DB"
}

vps_drop_old() {
  run_cmd "$PSQL" -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS $OLD_DB;"
  echo "OK: dropped $OLD_DB"
}

case "${1:-}" in
  local-dump) local_dump ;;
  upload) upload ;;
  vps-restore) vps_restore ;;
  vps-rollback) vps_rollback ;;
  vps-drop-old) vps_drop_old ;;
  *) usage ;;
esac
