#!/usr/bin/env bash
set -euo pipefail

# Phase 9 security hardening gate (SaaS VPS).
# Usage: ./scripts/security_hardening_gate.sh

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "▶ Phase 9 unit gates"
go test ./internal/perfvalidation/ -run 'TestPhase9' -count=1
go test ./internal/saas/ ./internal/saas/middleware/ ./internal/middleware/ ./cmd/api/ -short -count=1

echo "▶ Security toolchain"
go test ./... -short -count=1

echo "✓ Phase 9 security hardening gate passed"
