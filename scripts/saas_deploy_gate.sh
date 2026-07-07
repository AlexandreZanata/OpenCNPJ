#!/usr/bin/env bash
# Phase 10 gate — production deploy runbook artifacts and smoke path.
#
# Usage:
#   ./scripts/saas_deploy_gate.sh              # templates + unit tests
#   ./scripts/saas_deploy_gate.sh --docker     # + migrate, API, smoke
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

required_files=(
  docs/ops/DEPLOY-RUNBOOK.md
  deploy/saas/systemd-opencnpj-api.example
  deploy/saas/api.env.example
  deploy/saas/redis-opencnpj.conf.example
  deploy/saas/rollback.example.sh
  scripts/build_opencnpj_api.sh
  scripts/saas_smoke.sh
  config/config.saas.example.yaml
)

echo "==> Checking deploy runbook artifacts"
for f in "${required_files[@]}"; do
  if [[ ! -f "$ROOT/$f" ]]; then
    echo "MISSING: $f" >&2
    exit 1
  fi
done

echo "==> Checking SaaS migrations"
for f in \
  migrations/saas/000001_saas_metadata.up.sql \
  migrations/saas/000002_saas_indexes.up.sql \
  migrations/saas/000003_api_key_index_rename.up.sql \
  migrations/saas/000004_admin_seed.up.sql
do
  if [[ ! -f "$ROOT/$f" ]]; then
    echo "MISSING migration: $f" >&2
    exit 1
  fi
done

echo "==> Unit gates"
go test ./deploy/saas/... ./internal/perfvalidation/ -run 'TestPhase10' -count=1
go test ./deploy/saas/... -count=1

echo "==> Build script dry-run"
tmp_bin="$(mktemp /tmp/opencnpj-api-XXXX)"
"$ROOT/scripts/build_opencnpj_api.sh" "$tmp_bin"
rm -f "$tmp_bin"

if [[ "${1:-}" != "--docker" ]]; then
  echo "OK: Phase 10 deploy gate passed (run with --docker for integration)"
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker required for --docker mode" >&2
  exit 1
fi

echo "==> Docker integration: migrate + API + smoke"
CONTAINER="opencnpj-deploy-gate-$$"
PG_PORT=$((56440 + RANDOM % 1000))
API_PID=""
GATE_CONFIG="$(mktemp /tmp/opencnpj-deploy-XXXX.yaml)"

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
sed -e 's/port: 8081/port: 18082/' \
  -e 's/admin_enabled: true/admin_enabled: false/' \
  "$ROOT/config/config.saas.example.yaml" >"$GATE_CONFIG"
export CONFIG_FILE="$GATE_CONFIG"

go run ./cmd/migrate --saas

(cd "$ROOT" && go run ./cmd/api) &
API_PID=$!

for _ in $(seq 1 25); do
  if curl -sf "http://127.0.0.1:18082/readyz" >/dev/null 2>&1; then
    "$ROOT/scripts/saas_smoke.sh" "http://127.0.0.1:18082"
    echo "OK: Phase 10 deploy gate passed (docker)"
    exit 0
  fi
  sleep 1
done

fail_msg="API did not become ready on :18082"
echo "FAIL: $fail_msg" >&2
exit 1
