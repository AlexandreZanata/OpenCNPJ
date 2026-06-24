# Data Import

Import Receita Federal CSV files from a local `data/` directory into PostgreSQL.

**Quick start:** see [QUICKSTART.md](QUICKSTART.md) for single-command download and import.

## Prerequisites

- Docker PostgreSQL running (`docker compose up -d postgres`)
- CSV files in `./data` — `make download` fetches the latest month
- Migrations applied: `go run ./cmd/migrate`

## One-command workflows

| Command | What it does |
|---------|--------------|
| `make download` | Latest RFB data + terminal progress % |
| `make import-full` | 100% import + live rows/s logs + report |
| `make download-and-import` | Download → migrate → import (full pipeline) |

Hardware auto-tuning: [HARDWARE.md](HARDWARE.md)  
Benchmark (32 GB / RTX 4060): [benchmarks/HARDWARE-RTX4060-32GB.md](benchmarks/HARDWARE-RTX4060-32GB.md)

## Full import (100%)

Canonical script — drops secondary indexes, truncates fact tables, imports, rebuilds indexes, refreshes stats:

```bash
bash scripts/run_full_import.sh
```

Typical timing (215M rows, 8 workers, batch 100k):

| Phase | Duration |
|-------|----------|
| COPY ingest | ~13 min |
| Index rebuild | ~5 min |
| **Total** | **~18 min** |

Report saved to `/tmp/full_import_performance_report.txt`.

## Sample / benchmark imports

```bash
make import-sample                    # 10% sample
make benchmark-10pct                  # benchmark A01 @ 10%
make benchmark-all-approaches         # A01–A05 comparison suite
```

Configuration: `scripts/benchmark_approaches.conf`. Results: `docs/benchmarks/COMPARISON.md`.

## Manual importer flags

```bash
go run ./cmd/importer \
  --data-path=./data \
  --sample-percent=100 \
  --workers=8 \
  --batch-size=100000 \
  --tune \
  --skip-refs \
  --benchmark
```

| Flag | Description |
|------|-------------|
| `--sample-percent` | Import fraction (1–100) |
| `--workers` | Parallel file workers |
| `--batch-size` | COPY batch size |
| `--tune` | PG session tuning (synchronous_commit=off, etc.) |
| `--skip-refs` | Skip reference tables if already loaded |
| `--benchmark` | Print rows/s summary |

## Index management

```bash
bash scripts/drop_all_import_indexes.sh      # before import
bash scripts/recreate_indexes_after_import.sh # after import
bash scripts/finalize_import.sh               # VACUUM ANALYZE + indexes
bash scripts/refresh_stats_aggregates.sh      # analytics summary tables
```

## System guard

Heavy imports use memory guard to avoid OOM:

```bash
make guard-status
GUARD_ENABLED=true bash scripts/run_with_guard.sh go run ./cmd/importer ...
```

## Troubleshooting

**Duplicate key errors** — truncate fact tables before re-import:

```bash
docker compose exec -T postgres psql -U receita_user -d receita_db \
  -c "TRUNCATE simples, socios, estabelecimentos, empresas CASCADE;"
```

**Recreate PostgreSQL volume** (destructive):

```bash
docker compose down postgres
docker volume rm busca-cnpj-2026_postgres_data
docker compose up -d postgres
go run ./cmd/migrate
```
