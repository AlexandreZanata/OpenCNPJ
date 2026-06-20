# Benchmark Comparison — 5 Import Approaches

Generated: 2026-06-11T15:02:33

## Winner

- **10% fastest:** `A01` — Optimized Parallel
- **20% fastest:** `A01` — Optimized Parallel

## Results at 10%

| Rank | Approach | Wall (s) | Rows | RPS | vs best |
|------|----------|----------|------|-----|---------|
| 1 | A01 Optimized Parallel | 134.31 | 21,529,661 | 174,084 | +0.0% |
| 2 | A02 Sequential Files | 224.7 | 21,529,661 | 96,149 | +67.3% |

## Results at 20%

| Rank | Approach | Wall (s) | Rows | RPS | vs best |
|------|----------|----------|------|-----|---------|
| 1 | A01 Optimized Parallel | 186.18 | 43,080,941 | 231,805 | +0.0% |
| 2 | A02 Sequential Files | 348.11 | 43,080,941 | 124,274 | +87.0% |

## Scaling 10% → 20%

| Approach | 10% (s) | 20% (s) | Time +% | Rows +% | Linearity |
| A01 Optimized Parallel | 134.31 | 186.18 | +38.6% | +100.1% | 69.3% |
| A02 Sequential Files | 224.7 | 348.11 | +54.9% | +100.1% | 77.4% |

## Raw data

See `data/benchmark_comparison.tsv`
