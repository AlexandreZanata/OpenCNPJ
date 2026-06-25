#!/usr/bin/env bash
# Install OpenCNPJ PostgreSQL example snippets on VPS (native install).
# Copies *.example templates — edit on host before reload. Real configs are gitignored.
# Usage: sudo ./scripts/vps_apply_postgresql_conf.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONF_MAIN="$ROOT/deploy/vps/postgresql-opencnpj.conf.example"
CONF_AUTO="$ROOT/deploy/vps/postgresql-autovacuum-opencnpj.conf.example"
PG_CONF_D="${PG_CONF_D:-/etc/postgresql/18/main/conf.d}"

if [[ "$(id -u)" -ne 0 ]]; then
  echo "Run as root (sudo)." >&2
  exit 1
fi

for f in "$CONF_MAIN" "$CONF_AUTO"; do
  if [[ ! -f "$f" ]]; then
    echo "missing example template: $f" >&2
    exit 1
  fi
done

mkdir -p "$PG_CONF_D"
install -m 0644 "$CONF_MAIN" "$PG_CONF_D/99-opencnpj.conf"
install -m 0644 "$CONF_AUTO" "$PG_CONF_D/99-opencnpj-autovacuum.conf"

echo "Installed example templates (edit on host before reload):"
echo "  $PG_CONF_D/99-opencnpj.conf"
echo "  $PG_CONF_D/99-opencnpj-autovacuum.conf"
echo
echo "After editing: sudo systemctl reload postgresql"
echo "Then: $ROOT/scripts/vps_analyze_search_tables.sh"
echo "      STRICT_VPS=1 $ROOT/scripts/opencnpj_advanced_phase2.sh http://localhost:8080"
