#!/usr/bin/env bash
# Monthly CNPJ sync — EXAMPLE ONLY (run on local PC + VPS manually)
#
# This script documents the workflow. Copy to your workstation/VPS and edit
# placeholders. Do NOT commit real hosts, users, or passwords.
#
# See: .local/03-saas-vps-comerc-api/11-MONTHLY-CNPJ-SYNC.md
set -euo pipefail

# --- Local PC settings ---
LOCAL_DATABASE_URL="${LOCAL_DATABASE_URL:-postgres://receita_user:CHANGE_ME@localhost:5434/receita_db}"
DUMP_DIR="${DUMP_DIR:-$HOME/opencnpj-dumps}"
VPS_HOST="${VPS_HOST:-YOUR_VPS_IP}"
VPS_USER="${VPS_USER:-root}"
REMOTE_INCOMING="${REMOTE_INCOMING:-/var/lib/opencnpj/incoming}"

usage() {
  echo "Usage: $0 {local-dump|upload|vps-restore}"
  exit 1
}

local_dump() {
  mkdir -p "$DUMP_DIR"
  local tag
  tag="$(date +%Y%m)"
  local dump="$DUMP_DIR/opencnpj_cnpj_${tag}.dump"
  echo "=== Validate local DB ==="
  psql "$LOCAL_DATABASE_URL" -c "SELECT count(*) AS estabelecimentos FROM estabelecimentos"
  echo "=== pg_dump ==="
  pg_dump -Fc --no-owner --no-acl -f "$dump" "$LOCAL_DATABASE_URL"
  zstd -T0 -19 -f "$dump"
  sha256sum "${dump}.zst" | tee "${dump}.zst.sha256"
  echo "Done: ${dump}.zst"
}

upload() {
  local tag
  tag="$(date +%Y%m)"
  local file="$DUMP_DIR/opencnpj_cnpj_${tag}.dump.zst"
  rsync -avP "$file" "${file}.sha256" "${VPS_USER}@${VPS_HOST}:${REMOTE_INCOMING}/"
}

vps_restore() {
  echo "Run on VPS as root — see 11-MONTHLY-CNPJ-SYNC.md for full swap procedure"
  echo "  systemctl stop opencnpj-api"
  echo "  pg_restore into opencnpj_cnpj_new → validate → rename swap"
  echo "  systemctl start opencnpj-api"
}

case "${1:-}" in
  local-dump) local_dump ;;
  upload) upload ;;
  vps-restore) vps_restore ;;
  *) usage ;;
esac
