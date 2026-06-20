# A02 — Sequential Files

## Hypothesis

Single-worker import avoids connection contention and duplicate-key races, but loses parallelism across files — likely slower than A01.

## Configuration

| Parameter | Value |
|-----------|-------|
| Workers | 1 |
| Batch size | 100,000 |
| PG tuning (`--tune`) | yes |
| Drop secondary indexes | yes |
| Skip refs | yes |

## Mechanism

Same COPY pipeline as A01, but only one file processed at a time. Useful to measure overhead of parallel coordination vs raw single-thread throughput.

## Results

| Sample | Wall (s) | Rows | RPS | Run at |
|--------|----------|------|-----|--------|
| 10% | _pending_ | _pending_ | _pending_ | _pending_ |
| 20% | _pending_ | _pending_ | _pending_ | _pending_ |

## Command

```bash
APPROACH_ID=A02 SAMPLE_PERCENT=10 \
  IMPORT_WORKERS=1 IMPORT_BATCH_SIZE=100000 IMPORT_TUNE=true DROP_INDEXES=true \
  bash scripts/benchmark_import_sample.sh
```
