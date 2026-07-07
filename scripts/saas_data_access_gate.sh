#!/usr/bin/env bash
# Phase 12 gate — sqlc + pgx data-access stack (no ORM, index EXPLAIN proof).
#
# Usage:
#   ./scripts/saas_data_access_gate.sh              # templates + unit tests
#   ./scripts/saas_data_access_gate.sh --docker     # + EXPLAIN integration (CNPJ + SaaS)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v sqlc >/dev/null 2>&1; then
  SQLC="$(go env GOPATH)/bin/sqlc"
else
  SQLC="sqlc"
fi

echo "==> sqlc vet + generate (pinned v1.29.0)"
"$SQLC" vet
"$SQLC" generate
if ! git diff --quiet -- internal/db/cnpj internal/db/saas; then
  echo "sqlc output drift — run sqlc generate and commit" >&2
  git diff -- internal/db/cnpj internal/db/saas >&2 || true
  exit 1
fi

echo "==> Required Phase 12 files"
required=(
  sqlc.yaml
  docs/ops/DATA-ACCESS-PERFORMANCE.md
  db/schema/cnpj.sql
  db/queries/cnpj/estabelecimento.sql
  db/queries/cnpj/empresa.sql
  db/queries/cnpj/socios.sql
  db/queries/cnpj/simples.sql
  db/queries/saas/api_keys.sql
  db/queries/saas/api_usage.sql
  internal/database/cnpj_pgx.go
  internal/database/saas_pgx.go
  internal/cnpj/service.go
  migrations/saas/000001_saas_metadata.up.sql
  migrations/saas/000002_saas_indexes.up.sql
)
for f in "${required[@]}"; do
  if [[ ! -f "$ROOT/$f" ]]; then
    echo "MISSING: $f" >&2
    exit 1
  fi
done

echo "==> Index definitions in migrations"
for needle in \
  idx_estabelecimentos_cnpj_completo \
  idx_api_keys_hash \
  idx_api_clients_status_active \
  'PRIMARY KEY (client_id, date)'
do
  if ! grep -rqF "$needle" "$ROOT/db/schema" "$ROOT/migrations/saas"; then
    echo "MISSING index/PK reference: $needle" >&2
    exit 1
  fi
done

echo "==> Goroutine budget (errgroup fan-out)"
fanout="$(grep -c 'g\.Go(' "$ROOT/internal/cnpj/service.go" || true)"
if [[ "$fanout" -ne 4 ]]; then
  echo "CNPJ lookup fan-out = $fanout, want 4" >&2
  exit 1
fi

echo "==> Unit gates"
go test ./internal/perfvalidation/ -run TestPhase12 -count=1
go test ./internal/cnpj/... ./internal/db/cnpj/... ./internal/db/saas/... \
  ./internal/database/... ./internal/saas/... -short -count=1

if [[ "${1:-}" != "--docker" ]]; then
  echo "OK: Phase 12 data-access gate passed (run with --docker for EXPLAIN integration)"
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker required for --docker mode" >&2
  exit 1
fi

echo "==> EXPLAIN integration (CNPJ cnpj_completo index)"
go test ./internal/cnpj/... -run TestIntegration_EXPLAINCnpjCompletoIndex -count=1 -timeout 10m

echo "==> EXPLAIN integration (SaaS api_keys key_hash index)"
go test ./internal/saas/... -run TestIntegration_APIKeyLookupAndExplain -count=1 -timeout 10m

echo "OK: Phase 12 data-access gate passed (docker)"
