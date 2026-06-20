# A05 — No PG Tuning

## Hypothesis

Disabling session-level PostgreSQL tuning (`--tune`) keeps safer durability defaults but adds fsync/WAL overhead — measuring how much A01 gains from tuning alone.

## Configuration

| Parameter | Value |
|-----------|-------|
| Workers | 10 |
| Batch size | 100,000 |
| PG tuning (`--tune`) | **no** |
| Drop secondary indexes | yes |
| Skip refs | yes |

## Mechanism

Parallel COPY with indexes dropped, but without `synchronous_commit=off`, `session_replication_role=replica`, or elevated `work_mem`. Isolates the impact of PG session optimizations.

## Results

| Sample | Wall (s) | Rows | RPS | Run at |
|--------|----------|------|-----|--------|
| 10% | _pending_ | _pending_ | _pending_ | _pending_ |
| 20% | _pending_ | _pending_ | _pending_ | _pending_ |

## Command

```bash
APPROACH_ID=A05 SAMPLE_PERCENT=10 \
  IMPORT_WORKERS=10 IMPORT_BATCH_SIZE=100000 IMPORT_TUNE=false DROP_INDEXES=true \
  bash scripts/benchmark_import_sample.sh
```
