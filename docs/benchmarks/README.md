# Import Benchmarks

Compare bulk COPY ingest strategies. Generated results live in [COMPARISON.md](COMPARISON.md).

## Approaches (`scripts/benchmark_approaches.conf`)

| ID | Name | Workers | Batch | PG tune | Drop indexes |
|----|------|---------|-------|---------|--------------|
| A01 | Optimized Parallel | 10 | 100k | yes | yes |
| A02 | Sequential Files | 1 | 100k | yes | yes |
| A03 | Large Batch | 10 | 250k | yes | yes |
| A04 | Max Workers | 16 | 100k | yes | yes |
| A05 | No PG Tuning | 10 | 100k | no | yes |

**Winner (measured):** A01 — Optimized Parallel.

## Run

```bash
# Single approach @ 10%
APPROACH_ID=A01 SAMPLE_PERCENT=10 bash scripts/benchmark_import_sample.sh

# Full suite (A01–A05 @ 10% and 20%)
make benchmark-all-approaches
```

Raw TSV: `data/benchmark_comparison.tsv`

## Full import (100%)

```bash
make import-full
cat /tmp/full_import_performance_report.txt
```

Latest measured run: [2026-06-24-full-import-i7-13620H-31GB.md](2026-06-24-full-import-i7-13620H-31GB.md)

## API / frontend (VPS parity local)

Production-like Postgres + API config on workstation (see `docs/ops/LOCAL-VPS-PARITY.md`):

```bash
./scripts/local_vps_parity_stack.sh
make web-dev   # http://localhost:5173
```

Report: `docs/benchmarks/YYYY-MM-DD-vps-parity-local-frontend.md`

## Parser micro-benchmarks (CI)

```bash
make bench
# or: go test ./tests/benchmark/... -bench=. -benchmem
```

Targets: parse ≥ 200k rows/s, CSV decode ≥ 50k rows/s on GitHub runners.

## API search performance (P0–P2)

Validation report: [2026-06-24-api-search-performance.md](2026-06-24-api-search-performance.md)

```bash
# Automated gate (Phase 8)
./scripts/api_perf_validation.sh http://localhost:8080

# k6 (requires k6 installed)
k6 run .local/01-api-performance-optimization/benchmarks/k6-baseline.js
k6 run .local/01-api-performance-optimization/benchmarks/k6-keyset-deep.js
```

See also [docs/PERFORMANCE.md](../PERFORMANCE.md).
