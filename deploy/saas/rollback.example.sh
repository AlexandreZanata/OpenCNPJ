#!/usr/bin/env bash
# Rollback OpenCNPJ API to previous binary (EXAMPLE).
# Usage: sudo ./deploy/saas/rollback.example.sh
set -euo pipefail

BIN="/usr/local/bin/opencnpj-api"
BAK="${BIN}.bak"
SERVICE="opencnpj-api"

if [[ ! -f "$BAK" ]]; then
  echo "No backup at $BAK — aborting" >&2
  exit 1
fi

systemctl stop "$SERVICE"
cp "$BAK" "$BIN"
systemctl start "$SERVICE"
systemctl is-active --quiet "$SERVICE"
echo "OK: rolled back to $BAK"
