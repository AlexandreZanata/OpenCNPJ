# A03 — Large Batch

## Hypothesis

Larger COPY batches (250k) reduce transaction overhead and round-trips, improving throughput at the cost of higher memory per worker.

## Configuration

| Parameter | Value |
|-----------|-------|
| Workers | 10 |
| Batch size | 250,000 |
| PG tuning (`--tune`) | yes |
| Drop secondary indexes | yes |
| Skip refs | yes |

## Mechanism

Identical to A01 except batch size tripled. Tests whether amortizing commit/fsync cost across bigger batches beats smaller, more frequent commits.

## Results

| Sample | Wall (s) | Rows | RPS | Run at |
|--------|----------|------|-----|--------|
| 10% | _pending_ | _pending_ | _pending_ | _pending_ |
| 20% | _pending_ | _pending_ | _pending_ | _pending_ |

## Command

```bash
APPROACH_ID=A03 SAMPLE_PERCENT=10 \
  IMPORT_WORKERS=10 IMPORT_BATCH_SIZE=250000 IMPORT_TUNE=true DROP_INDEXES=true \
  bash scripts/benchmark_import_sample.sh
```
