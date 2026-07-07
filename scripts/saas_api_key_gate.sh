#!/usr/bin/env bash
# Phase 3 gate — API key auth + usage (unit + optional Docker integration)
#
# Usage:
#   ./scripts/saas_api_key_gate.sh              # unit tests only
#   ./scripts/saas_api_key_gate.sh --docker     # includes Postgres integration
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "==> sqlc generate (must be clean)"
if ! command -v sqlc >/dev/null 2>&1; then
  SQLC="$(go env GOPATH)/bin/sqlc"
else
  SQLC="sqlc"
fi
"$SQLC" generate
if ! git diff --quiet -- internal/db/saas; then
  echo "sqlc output drift — run sqlc generate and commit" >&2
  git diff -- internal/db/saas >&2 || true
  exit 1
fi

echo "==> Required SaaS files"
for f in \
  sqlc.yaml \
  db/queries/saas/api_keys.sql \
  db/queries/saas/api_usage.sql \
  internal/saas/api_key.go \
  internal/saas/usage.go \
  internal/saas/middleware/api_key.go \
  migrations/saas/000003_api_key_index_rename.up.sql
do
  if [[ ! -f "$ROOT/$f" ]]; then
    echo "MISSING: $f" >&2
    exit 1
  fi
done

echo "==> Unit tests (saas packages)"
go test ./internal/saas/... ./internal/database/... -short -count=1

if [[ "${1:-}" != "--docker" ]]; then
  echo "OK: API key unit gate passed (run with --docker for integration)"
  exit 0
fi

if ! command -v docker >/dev/null 2>&1; then
  echo "docker required for --docker mode" >&2
  exit 1
fi

echo "==> Integration test (testcontainers Postgres)"
go test ./internal/saas/... -run TestIntegration_APIKeyLookupAndExplain -count=1 -timeout 10m

echo "OK: Phase 3 API key gate passed"
