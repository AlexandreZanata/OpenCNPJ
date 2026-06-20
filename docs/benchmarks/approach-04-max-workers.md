# A04 — Max Workers

## Hypothesis

Matching worker count to CPU cores (16) maximizes I/O parallelism on a 16-core host, beating the conservative 10-worker default.

## Configuration

| Parameter | Value |
|-----------|-------|
| Workers | 16 |
| Batch size | 100,000 |
| PG tuning (`--tune`) | yes |
| Drop secondary indexes | yes |
| Skip refs | yes |

## Mechanism

Same as A01 but saturates all logical CPUs. Risk: Postgres connection/memory pressure or disk contention may hurt instead of help.

## Results

| Sample | Wall (s) | Rows | RPS | Run at |
|--------|----------|------|-----|--------|
| 10% | _pending_ | _pending_ | _pending_ | _pending_ |
| 20% | _pending_ | _pending_ | _pending_ | _pending_ |

## Command

```bash
APPROACH_ID=A04 SAMPLE_PERCENT=10 \
  IMPORT_WORKERS=16 IMPORT_BATCH_SIZE=100000 IMPORT_TUNE=true DROP_INDEXES=true \
  bash scripts/benchmark_import_sample.sh
```
