# A01 — Optimized Parallel (baseline)

## Hypothesis

Parallel file import with tuned PostgreSQL session settings and secondary indexes dropped is the best default for bulk COPY ingest.

## Configuration

| Parameter | Value |
|-----------|-------|
| Workers | 10 |
| Batch size | 100,000 |
| PG tuning (`--tune`) | yes |
| Drop secondary indexes | yes |
| Skip refs | yes |

## Mechanism

- Multiple CSV files imported concurrently (empresas, estabelecimentos, socios, simples).
- `COPY` batches of 100k rows per transaction.
- Session: `synchronous_commit=off`, `session_replication_role=replica`, elevated `work_mem`.
- UNIQUE constraints and secondary indexes removed before import; only PKs remain.

## Results

| Sample | Wall (s) | Rows | RPS | Run at |
|--------|----------|------|-----|--------|
| 10% | _pending_ | _pending_ | _pending_ | _pending_ |
| 20% | _pending_ | _pending_ | _pending_ | _pending_ |

## Command

```bash
APPROACH_ID=A01 SAMPLE_PERCENT=10 \
  IMPORT_WORKERS=10 IMPORT_BATCH_SIZE=100000 IMPORT_TUNE=true DROP_INDEXES=true \
  bash scripts/benchmark_import_sample.sh
```
