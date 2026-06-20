#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
exec python3 "$ROOT/scripts/api_audit.py" "${API_BASE:-http://localhost:8080}" "${AUDIT_DURATION:-20}" "${AUDIT_CONCURRENCY:-10}" "${AUDIT_REPORT:-/tmp/api_audit_report.txt}"
