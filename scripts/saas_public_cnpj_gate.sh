#!/usr/bin/env bash
# Phase 4 gate — public CNPJ API (sqlc + pgx + EXPLAIN)
#
# Usage:
#   ./scripts/saas_public_cnpj_gate.sh              # unit tests
#   ./scripts/saas_public_cnpj_gate.sh --docker     # + Postgres EXPLAIN integration
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v sqlc >/dev/null 2>&1; then
  SQLC="$(go env GOPATH)/bin/sqlc"
else
  SQLC="sqlc"
fi

echo "==> sqlc vet + generate"
"$SQLC" vet
"$SQLC" generate
if ! git diff --quiet -- internal/db/cnpj internal/db/saas; then
  echo "sqlc output drift — run sqlc generate and commit" >&2
  exit 1
fi

echo "==> Required Phase 4 files"
for f in \
  db/schema/cnpj.sql \
  db/queries/cnpj/estabelecimento.sql \
  internal/cnpj/service.go \
  internal/handlers/cnpj_handler.go
do
  if [[ ! -f "$ROOT/$f" ]]; then
    echo "MISSING: $f" >&2
    exit 1
  fi
done

echo "==> Unit tests"
go test ./internal/cnpj/... ./internal/db/cnpj/... ./internal/handlers/... -short -count=1

if [[ "${1:-}" != "--docker" ]]; then
  echo "OK: public CNPJ unit gate passed (run with --docker for EXPLAIN integration)"
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker required for --docker mode" >&2
  exit 1
fi

echo "==> EXPLAIN integration"
go test ./internal/cnpj/... -run TestIntegration_EXPLAINCnpjCompletoIndex -count=1 -timeout 10m

echo "OK: Phase 4 public CNPJ gate passed"
