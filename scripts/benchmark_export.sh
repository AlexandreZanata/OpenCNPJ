#!/usr/bin/env bash
# Benchmark CSV and phone export throughput (1k–500k rows).
# Usage: ./scripts/benchmark_export.sh [API_BASE_URL]
# Writes: docs/benchmarks/YYYY-MM-DD-export-throughput.md
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
DATE_TAG="$(date +%Y-%m-%d)"
REPORT="$ROOT/docs/benchmarks/${DATE_TAG}-export-throughput.md"
TMP_DIR="/tmp/export-bench-$$"
mkdir -p "$TMP_DIR"

CSV_LIMITS=(1000 5000 10000)
BULK_LIMIT=500000
COLS='["cnpj_completo","nome_fantasia","razao_social","cnae_fiscal_principal","uf","municipio"]'
PHONE_LIMITS=(1000 5000 10000)

echo "=== Export benchmark ==="
echo "API: $API_BASE"

if ! curl -sf --max-time 5 "${API_BASE}/readyz" >/dev/null; then
  echo "API not ready at $API_BASE" >&2
  exit 1
fi

bench_csv() {
  local limit="$1"
  local filter_json="$2"
  local label="$3"
  local timeout="$4"
  local out="$TMP_DIR/csv-${limit}.csv"
  local body
  body=$(printf '{"filters":%s,"selected_columns":%s,"format":"csv"}' \
    "$filter_json" "$COLS")
  local start end elapsed_ms lines bytes rps
  start=$(date +%s.%N)
  if ! curl -sf --max-time "$timeout" -X POST "${API_BASE}/api/v1/export/csv" \
    -H 'Content-Type: application/json' \
    -d "$body" -o "$out"; then
    echo "FAIL csv $label limit=$limit"
    return 1
  fi
  end=$(date +%s.%N)
  elapsed_ms=$(python3 -c "print(round((${end}-${start})*1000, 2))")
  lines=$(wc -l <"$out" | tr -d ' ')
  bytes=$(stat -c%s "$out" 2>/dev/null || stat -f%z "$out")
  local data_rows=$((lines > 0 ? lines - 1 : 0))
  rps=$(python3 -c "e=${elapsed_ms}/1000; print(round(${data_rows}/e, 1) if e>0 else 0)")
  printf '%s|%s|%s|%s|%s|%s\n' "$label" "$limit" "$elapsed_ms" "$data_rows" "$bytes" "$rps"
}

bench_phones() {
  local limit="$1"
  local out="$TMP_DIR/phones-${limit}.csv"
  local body
  body=$(printf '{"category":"restaurante","uf":"SP","only_active":true,"limit":%s,"format":"csv"}' "$limit")
  local start end elapsed_ms lines bytes rps
  start=$(date +%s.%N)
  if ! curl -sf --max-time 600 -X POST "${API_BASE}/api/v1/export/phones" \
    -H 'Content-Type: application/json' \
    -d "$body" -o "$out"; then
    echo "FAIL phones limit=$limit"
    return 1
  fi
  end=$(date +%s.%N)
  elapsed_ms=$(python3 -c "print(round((${end}-${start})*1000, 2))")
  lines=$(wc -l <"$out" | tr -d ' ')
  bytes=$(stat -c%s "$out" 2>/dev/null || stat -f%z "$out")
  local data_rows=$((lines > 0 ? lines - 1 : 0))
  rps=$(python3 -c "e=${elapsed_ms}/1000; print(round(${data_rows}/e, 1) if e>0 else 0)")
  printf 'phones_sp_restaurante|%s|%s|%s|%s|%s\n' "$limit" "$elapsed_ms" "$data_rows" "$bytes" "$rps"
}

FILTER_CNAE='{"uf":"SP","situacao_cadastral":"02","cnae_principal":"4781400","limit":%s}'
FILTER_BULK='{"uf":"SP","situacao_cadastral":"02","limit":%s}'

curl -sf --max-time 120 -X POST "${API_BASE}/api/v1/export/csv" \
  -H 'Content-Type: application/json' \
  -d '{"filters":{"uf":"SP","situacao_cadastral":"02","limit":100},"selected_columns":["cnpj_completo","uf"],"format":"csv"}' \
  -o /dev/null || true

RESULTS="$TMP_DIR/results.tsv"
: >"$RESULTS"

echo "--- CSV estabelecimentos (UF=SP, CNAE=4781400, active) ---"
for lim in "${CSV_LIMITS[@]}"; do
  filt=$(printf "$FILTER_CNAE" "$lim")
  bench_csv "$lim" "$filt" "csv_cnae" 600 | tee -a "$RESULTS"
done

echo "--- CSV bulk (UF=SP, active, limit=500000) ---"
filt_bulk=$(printf "$FILTER_BULK" "$BULK_LIMIT")
bench_csv "$BULK_LIMIT" "$filt_bulk" "csv_bulk_sp" 3600 | tee -a "$RESULTS"

echo "--- Phone export (category=restaurante, UF=SP, active) ---"
for lim in "${PHONE_LIMITS[@]}"; do
  bench_phones "$lim" | tee -a "$RESULTS"
done

ROW_COUNT=$(docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SELECT count(*) FROM estabelecimentos" 2>/dev/null | tr -d ' ' || echo "?")
PG_PROFILE=$(docker exec receita-postgres psql -U receita_user -d receita_db -tAc \
  "SELECT pg_get_partkeydef('estabelecimentos'::regclass)" 2>/dev/null | tr -d ' ' || echo "?")

python3 - "$REPORT" "$RESULTS" "$API_BASE" "$ROW_COUNT" "$PG_PROFILE" <<'PY'
import sys
from datetime import datetime

report_path, tsv_path, api_base, row_count, pg_profile = sys.argv[1:6]
rows = []
with open(tsv_path, encoding="utf-8") as fh:
    for line in fh:
        parts = line.strip().split("|")
        if len(parts) >= 6:
            rows.append(parts)

def table_for(kind_exact=None, kind_prefix=None):
    hdr = ["| Limit | Time (ms) | Rows | Size (MB) | Rows/s |", "|-------|-----------|------|-----------|--------|"]
    body = []
    for kind, limit, ms, data_rows, nbytes, rps in rows:
        if kind_exact and kind != kind_exact:
            continue
        if kind_prefix and not kind.startswith(kind_prefix):
            continue
        mb = round(int(nbytes) / (1024 * 1024), 2)
        body.append(f"| {limit} | {ms} | {data_rows} | {mb} | {rps} |")
    return "\n".join(hdr + body) if body else "_no data_"

md = f"""# Export throughput benchmark (VPS parity local)

- **Date**: {datetime.now().isoformat(timespec="seconds")}
- **API**: {api_base}
- **Config**: `config/config.vps-parity.yaml` · Postgres VPS profile
- **Dataset**: {row_count} estabelecimentos · partitioning: {pg_profile}

## Changes (this benchmark run)

| Change | Path |
|--------|------|
| CSV export max raised to **500,000** rows/request | `internal/repository/export_limits.go` |
| Export handler applies `NormalizeExportLimit` | `internal/handlers/export_handler.go` |
| Benchmark script + 500k bulk test | `scripts/benchmark_export.sh` |
| VPS parity local stack (import + PG profile) | `scripts/local_vps_parity_stack.sh` |
| Frontend/API parity config | `config/config.vps-parity.yaml` |

## CSV export — filtered CNAE (`uf=SP`, `cnae=4781400`, active)

6 columns: cnpj_completo, nome_fantasia, razao_social, cnae, uf, municipio.

{table_for(kind_exact="csv_cnae")}

## CSV bulk export — 500k (`uf=SP`, `situacao_cadastral=02`, `limit=500000`)

{table_for(kind_exact="csv_bulk_sp")}

## Phone export (`POST /api/v1/export/phones`)

Filter: `category=restaurante`, `uf=SP`, `only_active=true`.

{table_for(kind_prefix="phones")}

## Reproduce

```bash
go test ./internal/repository/... -short -run NormalizeExportLimit
./scripts/benchmark_export.sh http://localhost:8080
```

## Limits

| Endpoint | Default | Max |
|----------|---------|-----|
| `POST /api/v1/export/csv` | 10,000 | **500,000** |
| `POST /api/v1/export/phones` | 5,000 | 50,000 |

## Notes

- Frontend `ExportPanel` still sends `limit: 1000`; raise in UI for larger exports.
- Use UF (+ CNAE when possible) for LIST(uf) partition pruning.
- 500k export streams via `streamCSV` (row-by-row); expect minutes, not seconds.
- Phone export first request after idle may be slow (cold plan).
"""
with open(report_path, "w", encoding="utf-8") as fh:
    fh.write(md)
print(f"Report: {report_path}")
PY

rm -rf "$TMP_DIR"
echo "=== done ==="
