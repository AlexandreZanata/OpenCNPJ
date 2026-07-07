#!/usr/bin/env bash
# Phase 2 gate — dual PostgreSQL databases (opencnpj_cnpj + opencnpj_saas)
#
# Usage:
#   ./scripts/saas_dual_db_gate.sh              # template + repo checks only
#   ./scripts/saas_dual_db_gate.sh --docker     # full integration via Docker
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BOOTSTRAP="$ROOT/deploy/saas/postgres-bootstrap.sql.example"
API_ENV="$ROOT/deploy/saas/api.env.example"

required_bootstrap=(
  'opencnpj_cnpj'
  'opencnpj_saas'
  'opencnpj_reader'
  'opencnpj_saas'
  'opencnpj_restore'
  'GRANT SELECT ON ALL TABLES'
)

echo "==> Checking bootstrap SQL template"
for needle in "${required_bootstrap[@]}"; do
  if ! grep -qF "$needle" "$BOOTSTRAP"; then
    echo "MISSING in bootstrap: $needle" >&2
    exit 1
  fi
done

echo "==> Checking api.env.example"
for var in CNPJ_DATABASE_URL SAAS_DATABASE_URL REDIS_URL CONFIG_FILE; do
  if ! grep -qF "$var=" "$API_ENV"; then
    echo "MISSING env var: $var" >&2
    exit 1
  fi
done

echo "==> Checking SaaS migrations"
for f in 000001_saas_metadata.up.sql 000002_saas_indexes.up.sql 000003_api_key_index_rename.up.sql; do
  if [[ ! -f "$ROOT/migrations/saas/$f" ]]; then
    echo "MISSING migration: $f" >&2
    exit 1
  fi
done

if [[ "${1:-}" != "--docker" ]]; then
  echo "OK: dual-database templates valid (run with --docker for integration)"
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker required for --docker mode" >&2
  exit 1
fi

echo "==> Docker integration gate"
CONTAINER="opencnpj-dualdb-gate-$$"
PG_PORT=$((55440 + RANDOM % 1000))
API_PID=""
GATE_CONFIG="$(mktemp /tmp/opencnpj-gate-XXXX.yaml)"

cleanup() {
  kill "$API_PID" 2>/dev/null || true
  rm -f "$GATE_CONFIG"
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run -d --name "$CONTAINER" \
  -e POSTGRES_PASSWORD=postgres \
  -p "${PG_PORT}:5432" \
  postgres:18-alpine >/dev/null

for _ in $(seq 1 30); do
  if docker exec "$CONTAINER" pg_isready -U postgres >/dev/null 2>&1; then
    break
  fi
  sleep 1
done

docker exec -i "$CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 <<'SQL'
CREATE ROLE opencnpj_reader LOGIN PASSWORD 'reader_pass';
CREATE ROLE opencnpj_saas LOGIN PASSWORD 'saas_pass';
CREATE DATABASE opencnpj_cnpj;
CREATE DATABASE opencnpj_saas OWNER opencnpj_saas;
GRANT CONNECT ON DATABASE opencnpj_cnpj TO opencnpj_reader;
\c opencnpj_cnpj
GRANT USAGE ON SCHEMA public TO opencnpj_reader;
\c postgres
GRANT ALL PRIVILEGES ON DATABASE opencnpj_saas TO opencnpj_saas;
\c opencnpj_saas
GRANT ALL ON SCHEMA public TO opencnpj_saas;
SQL

export SAAS_DATABASE_URL="postgres://opencnpj_saas:saas_pass@127.0.0.1:${PG_PORT}/opencnpj_saas?sslmode=disable"
export CNPJ_DATABASE_URL="postgres://opencnpj_reader:reader_pass@127.0.0.1:${PG_PORT}/opencnpj_cnpj?sslmode=disable"
sed 's/port: 8081/port: 18081/' "$ROOT/config/config.saas.example.yaml" >"$GATE_CONFIG"
export CONFIG_FILE="$GATE_CONFIG"

echo "==> Applying SaaS migrations"
(cd "$ROOT" && go run ./cmd/migrate --saas)

echo "==> Verifying SaaS tables"
count="$(docker exec "$CONTAINER" psql -U opencnpj_saas -d opencnpj_saas -tAc 'SELECT count(*) FROM api_clients')"
if [[ "$count" != "0" ]]; then
  echo "expected 0 api_clients, got $count" >&2
  exit 1
fi

idx_count="$(docker exec "$CONTAINER" psql -U opencnpj_saas -d opencnpj_saas -tAc \
  "SELECT count(*) FROM pg_indexes WHERE indexname IN ('idx_api_keys_hash','idx_api_clients_status_active')")"
if [[ "$idx_count" -lt 2 ]]; then
  echo "expected SaaS performance indexes, found $idx_count" >&2
  exit 1
fi

echo "==> Starting API (brief) for /readyz"
(cd "$ROOT" && go run ./cmd/api) &
API_PID=$!

for _ in $(seq 1 20); do
  code="$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:18081/readyz 2>/dev/null || true)"
  if [[ "$code" == "200" ]]; then
    echo "/readyz => 200"
    kill "$API_PID" 2>/dev/null || true
    wait "$API_PID" 2>/dev/null || true
    echo "OK: Phase 2 dual-database gate passed"
    exit 0
  fi
  sleep 1
done

echo "FAIL: /readyz did not return 200" >&2
exit 1
