#!/usr/bin/env bash
# Frontend-oriented API benchmark under VPS parity (writes docs/benchmarks report).
# Usage: ./scripts/local_vps_frontend_benchmark.sh [API_BASE_URL]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
DATE_TAG="$(date +%Y-%m-%d)"
REPORT_MD="$ROOT/docs/benchmarks/${DATE_TAG}-vps-parity-local-frontend.md"
AUDIT_TXT="/tmp/vps_parity_frontend_audit.txt"
DURATION="${BENCH_DURATION:-15}"
CONCURRENCY="${BENCH_CONCURRENCY:-8}"

pick_sample() {
  docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
    "SELECT e.cnpj_completo FROM estabelecimentos e WHERE length(e.cnpj_completo)=14 LIMIT 1" \
    2>/dev/null | tr -d '[:space:]'
}

measure_ms() {
  local url="$1"
  curl -s -o /dev/null -w "%{time_total}" --max-time 120 "$url" 2>/dev/null \
    | python3 -c "import sys; print(round(float(sys.stdin.read() or 0)*1000, 2))"
}

echo "=== VPS parity frontend benchmark ==="
echo "API: $API_BASE"

if ! curl -sf --max-time 5 "${API_BASE}/readyz" >/dev/null; then
  echo "API not ready — start: ./scripts/local_vps_parity_api.sh" >&2
  exit 1
fi

CNPJ="$(pick_sample)"
if [[ -z "$CNPJ" ]]; then
  echo "no sample CNPJ in database" >&2
  exit 1
fi

# Warm L1 + Redis
curl -sf "${API_BASE}/api/v1/estabelecimentos/${CNPJ}" >/dev/null || true
curl -sf "${API_BASE}/api/v1/empresas/search?razao_social=PETROBRAS&limit=5" >/dev/null || true

python3 "$ROOT/scripts/api_audit.py" "$API_BASE" "$DURATION" "$CONCURRENCY" "$AUDIT_TXT" || true

PG_PROFILE=$(docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SELECT pg_get_partkeydef('estabelecimentos'::regclass)" 2>/dev/null || echo "unknown")
SHARED_BUFFERS=$(docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SHOW shared_buffers" 2>/dev/null || echo "unknown")
WORK_MEM=$(docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SHOW work_mem" 2>/dev/null || echo "unknown")
ROW_COUNT=$(docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SELECT count(*) FROM estabelecimentos" 2>/dev/null || echo "0")

# Single-request latency (mirrors web UI flows)
LAT_CNPJ=$(measure_ms "${API_BASE}/api/v1/estabelecimentos/${CNPJ}")
LAT_EMPRESA=$(measure_ms "${API_BASE}/api/v1/empresas/search?razao_social=PETROBRAS&limit=10")
LAT_ESTAB_UF=$(measure_ms "${API_BASE}/api/v1/estabelecimentos/search?uf=SP&nome_fantasia=PADARIA&limit=10")
LAT_LOOKUP=$(measure_ms "${API_BASE}/api/v1/lookup/cnae?q=comerc&limit=10")
LAT_STATS=$(measure_ms "${API_BASE}/api/v1/stats/uf")
LAT_ANALYTICS=$(measure_ms "${API_BASE}/api/v1/analytics/summary?cnae_limit=5")

HOST_RAM=$(free -h | awk '/^Mem:/{print $2}')
HOST_CPU=$(nproc)

cat >"$REPORT_MD" <<EOF
# VPS parity — local frontend API benchmark

- **Date**: $(date -Iseconds)
- **Host**: ${HOST_CPU} cores, ${HOST_RAM} RAM
- **API config**: \`config/config.vps-parity.yaml\` (rate limit ON, L1 ON, \`BENCHMARK_MODE\` unset)
- **PostgreSQL**: VPS 16 GB profile via \`docker-compose.vps-parity.yml\`
- **Partitioning**: ${PG_PROFILE}
- **Rows (estabelecimentos)**: ${ROW_COUNT}
- **GUCs**: shared_buffers=${SHARED_BUFFERS}, work_mem=${WORK_MEM}

## Single-request latency (warm cache, ms)

| Web flow | Endpoint | ms |
|----------|----------|-----|
| CNPJ detail | \`GET /estabelecimentos/:cnpj\` | ${LAT_CNPJ} |
| Empresa search | \`GET /empresas/search?razao_social=...\` | ${LAT_EMPRESA} |
| Estabelecimento UF+text | \`GET /estabelecimentos/search?uf=SP&nome_fantasia=...\` | ${LAT_ESTAB_UF} |
| Lookup typeahead | \`GET /lookup/cnae?q=...\` | ${LAT_LOOKUP} |
| Stats dashboard | \`GET /stats/uf\` | ${LAT_STATS} |
| Analytics | \`GET /analytics/summary\` | ${LAT_ANALYTICS} |

## Sustained load (${DURATION}s @ concurrency=${CONCURRENCY})

\`\`\`
$(cat "$AUDIT_TXT" 2>/dev/null || echo "(api_audit skipped)")
\`\`\`

## Reproduce

\`\`\`bash
./scripts/local_vps_parity_stack.sh        # clean import + VPS PG profile
./scripts/local_vps_parity_api.sh          # terminal 1
make web-dev                               # terminal 2 → http://localhost:5173
./scripts/local_vps_frontend_benchmark.sh http://localhost:8080
\`\`\`

## Notes

- Import uses base \`docker-compose.yml\` (fast COPY flags); API tests use VPS production Postgres GUCs.
- Avoid \`?uf=SP&limit=N\` without text/CNAE — triggers full-partition \`COUNT(*)\` on large datasets.
EOF

echo "Report: $REPORT_MD"
cat "$AUDIT_TXT" 2>/dev/null || true
