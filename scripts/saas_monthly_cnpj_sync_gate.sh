#!/usr/bin/env bash
# Phase 11 gate — monthly CNPJ sync artifacts and swap procedure.
#
# Usage:
#   ./scripts/saas_monthly_cnpj_sync_gate.sh              # templates + unit tests
#   ./scripts/saas_monthly_cnpj_sync_gate.sh --docker     # + dump/restore/swap cycle
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SYNC_SCRIPT="$ROOT/deploy/saas/monthly-cnpj-sync.example.sh"
GRANT_SQL="$ROOT/deploy/saas/grant-reader.sql.example"
RUNBOOK="$ROOT/docs/ops/MONTHLY-CNPJ-SYNC.md"

required_files=(
  "$SYNC_SCRIPT"
  "$GRANT_SQL"
  "$RUNBOOK"
)

echo "==> Checking Phase 11 artifacts"
for f in "${required_files[@]}"; do
  if [[ ! -f "$f" ]]; then
    echo "MISSING: $f" >&2
    exit 1
  fi
done

echo "==> Checking sync script commands"
for needle in \
  local-dump upload vps-restore vps-rollback vps-drop-old \
  opencnpj_cnpj_new opencnpj_cnpj_old grant-reader.sql 'cnpj:*'
do
  if ! grep -qF "$needle" "$SYNC_SCRIPT"; then
    echo "MISSING in sync script: $needle" >&2
    exit 1
  fi
done

echo "==> Checking grant-reader SQL"
for needle in opencnpj_reader 'GRANT SELECT ON ALL TABLES'; do
  if ! grep -qF "$needle" "$GRANT_SQL"; then
    echo "MISSING in grant SQL: $needle" >&2
    exit 1
  fi
done

bash -n "$SYNC_SCRIPT"

echo "==> Unit gates"
go test ./deploy/saas/... -run 'TestMonthly|TestGrantReader' -count=1

if [[ "${1:-}" != "--docker" ]]; then
  echo "OK: Phase 11 monthly CNPJ sync gate passed (run with --docker for integration)"
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker required for --docker mode" >&2
  exit 1
fi

echo "==> Docker integration: dump → restore → swap (SaaS DB untouched)"
CONTAINER="opencnpj-sync-gate-$$"
PG_PORT=$((57440 + RANDOM % 1000))
WORKDIR="$(mktemp -d /tmp/opencnpj-sync-XXXX)"
GRANT_FILE="$WORKDIR/grant-reader.sql"
DUMP_FILE="$WORKDIR/opencnpj_cnpj_test.dump"

cleanup() {
  rm -rf "$WORKDIR"
  docker rm -f "$CONTAINER" >/dev/null 2>&1 || true
}
trap cleanup EXIT

cp "$GRANT_SQL" "$GRANT_FILE"

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
CREATE TABLE estabelecimentos (id int PRIMARY KEY);
INSERT INTO estabelecimentos VALUES (1);
GRANT USAGE ON SCHEMA public TO opencnpj_reader;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO opencnpj_reader;
\c opencnpj_saas
CREATE TABLE api_clients (id serial PRIMARY KEY, name text NOT NULL);
INSERT INTO api_clients (name) VALUES ('gate-client');
GRANT ALL ON SCHEMA public TO opencnpj_saas;
SQL

saas_before="$(docker exec "$CONTAINER" psql -U postgres -tAc 'SELECT count(*) FROM api_clients' -d opencnpj_saas | tr -d '[:space:]')"
if [[ "$saas_before" != "1" ]]; then
  echo "expected 1 api_clients before sync, got $saas_before" >&2
  exit 1
fi

docker exec "$CONTAINER" pg_dump -U postgres -Fc --no-owner --no-acl \
  -f /tmp/opencnpj_cnpj_test.dump opencnpj_cnpj
docker cp "$CONTAINER:/tmp/opencnpj_cnpj_test.dump" "$DUMP_FILE" >/dev/null

docker exec "$CONTAINER" psql -U postgres -d opencnpj_cnpj \
  -c "UPDATE estabelecimentos SET id = 99 WHERE id = 1;"

docker exec "$CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -c "DROP DATABASE IF EXISTS opencnpj_cnpj_new;"
docker exec "$CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 -c "CREATE DATABASE opencnpj_cnpj_new;"
docker cp "$DUMP_FILE" "$CONTAINER:/tmp/opencnpj_cnpj_test.dump" >/dev/null
docker exec "$CONTAINER" pg_restore -U postgres --no-owner --no-acl \
  -d opencnpj_cnpj_new /tmp/opencnpj_cnpj_test.dump

staging_count="$(docker exec "$CONTAINER" psql -U postgres -tAc \
  'SELECT count(*) FROM estabelecimentos' -d opencnpj_cnpj_new | tr -d '[:space:]')"
if [[ "$staging_count" != "1" ]]; then
  echo "staging restore count expected 1, got $staging_count" >&2
  exit 1
fi

docker exec -i "$CONTAINER" psql -U postgres -v ON_ERROR_STOP=1 <<'SQL'
SELECT pg_terminate_backend(pid) FROM pg_stat_activity
WHERE datname = 'opencnpj_cnpj' AND pid <> pg_backend_pid();
ALTER DATABASE opencnpj_cnpj RENAME TO opencnpj_cnpj_old;
ALTER DATABASE opencnpj_cnpj_new RENAME TO opencnpj_cnpj;
SQL

docker cp "$GRANT_FILE" "$CONTAINER:/tmp/grant-reader.sql" >/dev/null
docker exec "$CONTAINER" psql -U postgres -d opencnpj_cnpj -f /tmp/grant-reader.sql >/dev/null
restored_id="$(docker exec "$CONTAINER" psql -U postgres -tAc \
  'SELECT id FROM estabelecimentos LIMIT 1' -d opencnpj_cnpj | tr -d '[:space:]')"
if [[ "$restored_id" != "1" ]]; then
  echo "expected restored id=1, got $restored_id" >&2
  exit 1
fi

saas_after="$(docker exec "$CONTAINER" psql -U postgres -tAc \
  'SELECT count(*) FROM api_clients' -d opencnpj_saas | tr -d '[:space:]')"
if [[ "$saas_after" != "$saas_before" ]]; then
  echo "opencnpj_saas changed during CNPJ sync: before=$saas_before after=$saas_after" >&2
  exit 1
fi

echo "OK: Phase 11 monthly CNPJ sync gate passed (docker)"
