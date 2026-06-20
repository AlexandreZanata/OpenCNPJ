# Performance

## Import (PostgreSQL COPY)

Session tuning (`--tune`):

- `synchronous_commit=off`
- `session_replication_role=replica`
- Elevated `work_mem` / `maintenance_work_mem`

Pipeline:

1. Drop secondary indexes (`scripts/drop_all_import_indexes.sh`)
2. Parallel COPY (`go run ./cmd/importer`)
3. Rebuild indexes + `ANALYZE`

**Measured (100% dataset, 2026-06-20):** ~286k rows/s ingest, ~13 min COPY + ~5 min indexes.

See [IMPORT.md](IMPORT.md) and [benchmarks/COMPARISON.md](benchmarks/COMPARISON.md).

## Go runtime

- `GOMAXPROCS` = CPU count
- CSV read buffer: 4 MB
- `GOGC=200` for long imports (optional)

## API targets

| Route | Target |
|-------|--------|
| CNPJ lookup | < 10 ms |
| Filtered search (cached) | < 100 ms |
| Analytics summary | < 100 ms (pre-aggregated) |
| Phone export (50k rows) | streaming, no full memory load |

## Pre-import checklist

- [ ] Secondary indexes dropped
- [ ] Sufficient disk for WAL + indexes (~2× table size headroom)
- [ ] System guard enabled for parallel workers on low-RAM hosts
