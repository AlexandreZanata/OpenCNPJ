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

## Parser micro-benchmarks (CI)

```bash
make bench
# or: go test ./tests/benchmark/... -bench=. -benchmem
```

Targets: parse ≥ 200k rows/s, CSV decode ≥ 50k rows/s on GitHub runners.
