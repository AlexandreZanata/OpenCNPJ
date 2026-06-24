# Import Benchmark — RTX 4060 / 32 GB RAM

Measured import performance for the reference workstation profile below.

**Date:** 2026-06-20 (full dataset)  
**Status:** baseline from project benchmarks; re-run with `make import-full` to refresh.

---

## Hardware

| Component | Specification |
|-----------|---------------|
| GPU | NVIDIA GeForce RTX 4060 (8 GB VRAM) |
| RAM | 32 GB DDR |
| Storage | NVMe SSD (recommended) |
| CPU | Multi-core (GOMAXPROCS = core count) |

> GPU is **not utilized** by the import pipeline. Performance is dominated by PostgreSQL COPY, parser throughput, and disk I/O.

---

## Software stack

| Component | Version / setting |
|-----------|-------------------|
| PostgreSQL | 18.4-alpine (Docker) |
| `shared_buffers` | 16 GB |
| `effective_cache_size` | 24 GB |
| `maintenance_work_mem` | 10 GB |
| Go importer | `--tune --benchmark --profile` |
| Workers | 8 |
| Batch size | 100,000 |
| Sample | 100% (~215M rows across fact tables) |

---

## Results — 100% full import

| Phase | Wall time | Throughput |
|-------|-----------|------------|
| COPY ingest | **~13 min** (~780 s) | **~286,000 rows/s** |
| Index rebuild + ANALYZE | **~5 min** (~300 s) | — |
| **Total (import + indexes)** | **~18 min** (~1,080 s) | **~199,000 rows/s effective** |

### Row counts (approximate)

| Table | Rows |
|-------|------|
| empresas | ~63M |
| estabelecimentos | ~71M |
| socios | ~26M |
| simples | ~46M |
| **Total** | **~206M+** |

### 10% sample benchmark (same hardware class)

From [COMPARISON.md](COMPARISON.md) — approach A01 (optimized parallel):

| Sample | Wall (s) | Rows | Rows/s |
|--------|----------|------|--------|
| 10% | 134.31 | 21,529,661 | 174,084 |
| 20% | 186.18 | 43,080,941 | 231,805 |

Scaling 10% → 20% is ~69% linear (good parallel efficiency).

---

## Download phase (same machine, network dependent)

| Metric | Typical range |
|--------|---------------|
| Total compressed ZIP | ~6 GB |
| Extracted CSV | ~35 GB |
| Download time @ 100 Mbps | 45–90 min |
| Download time @ 500 Mbps | 15–30 min |

Use `make download` — progress bar shows `[file/total] percentage`.

---

## Recommended settings (copy-paste)

```bash
# Auto-detected for 32 GB RAM
export IMPORT_WORKERS=8
export IMPORT_BATCH_SIZE=100000
export GOMAXPROCS=$(nproc)

make download-and-import
```

Report path after import: `/tmp/full_import_performance_report.txt`

---

## How to reproduce

```bash
make setup
make download          # or skip if ./data already populated
make import-full
cat /tmp/full_import_performance_report.txt
```

---

## Tuning headroom

| Change | Expected effect |
|--------|-----------------|
| `IMPORT_WORKERS=10` | +5–10% if CPU/disk not saturated |
| `IMPORT_WORKERS=12` | May trigger memory pressure on 32 GB |
| NVMe → SATA SSD | −20–40% rows/s |
| 16 GB RAM (reduce PG buffers) | −15–25% rows/s |
| `GUARD_ENABLED=true` | Safer; may throttle workers under pressure |

---

## API latency (post-import)

| Route | Target | Notes |
|-------|--------|-------|
| CNPJ lookup | < 10 ms | B-tree on `cnpj_completo` |
| Filtered search (cached) | < 100 ms | Redis + pg_trgm |
| Analytics summary | < 100 ms | Pre-aggregated tables |

See [PERFORMANCE.md](../PERFORMANCE.md).
