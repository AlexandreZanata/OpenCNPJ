# Quick Start

Get from zero to a running CNPJ search API in three commands.

## Prerequisites

| Requirement | Minimum |
|-------------|---------|
| Docker + Compose | PostgreSQL 18, Redis |
| Go | 1.21+ |
| Disk | ~50 GB free (`./data` + PostgreSQL volume) |
| RAM | 16 GB (32 GB recommended for full import) |

```bash
cp .env.example .env
make setup          # docker + .env + data/
go run ./cmd/migrate
```

---

## Option A — Download only (latest month)

Single command. Shows **live download percentage** in the terminal.

```bash
make download
# or
bash scripts/download_latest.sh
```

What it does:

1. Picks the **latest published month** on Receita Federal (falls back if current month is not ready)
2. Downloads all ZIP archives to `./data`
3. Extracts CSV files automatically
4. Skips files already downloaded (safe to re-run)

Progress line example:

```text
[12/37]  32.4%  Empresas3.zip  (142.3 MB / 410.0 MB)
```

List available months without downloading:

```bash
make list-months
```

---

## Option B — Import only (CSV already in `./data`)

Single command. Shows **live import performance** (rows/s every 10 seconds).

```bash
make import-full
# or
bash scripts/run_full_import.sh
```

What it does:

1. Starts PostgreSQL
2. Drops secondary indexes (faster COPY)
3. Imports 100% of CSV data
4. Rebuilds indexes + `ANALYZE`
5. Saves report to `/tmp/full_import_performance_report.txt`

Live log example:

```text
[2026-06-24 10:15:30] import | 45230100 rows | 286412 rows/s | 48.2 MB/s | 0 errors
```

Row-count monitor (every 5 s) → `/tmp/import_progress.log`

---

## Option C — Everything in one command

Download latest data **and** import with performance logs:

```bash
make download-and-import
# or
bash scripts/download_and_import.sh
```

Steps: download → migrate → full import → performance report.

Estimated time on **32 GB RAM** (see [benchmarks/HARDWARE-RTX4060-32GB.md](benchmarks/HARDWARE-RTX4060-32GB.md)):

| Phase | Time |
|-------|------|
| Download | 30–90 min (depends on network) |
| Import COPY | ~13 min |
| Index rebuild | ~5 min |
| **Total import** | **~18 min** |

---

## Start the API + web UI

```bash
# Terminal 1 — API
go run ./cmd/api

# Terminal 2 — Web portal
make web-dev
```

Open http://localhost:5173 — API at http://localhost:8080

---

## Hardware tuning

The import scripts auto-detect RAM and set workers:

| RAM | Workers | Batch size |
|-----|---------|------------|
| ≥ 28 GB | 8 | 100,000 |
| 14–27 GB | 6 | 75,000 |
| < 14 GB | 4 | 50,000 |

Override manually:

```bash
IMPORT_WORKERS=10 IMPORT_BATCH_SIZE=100000 make import-full
```

Full tuning guide: [HARDWARE.md](HARDWARE.md)

---

## Common issues

| Problem | Fix |
|---------|-----|
| `no monthly directories found` | Check internet; RFB server may be down |
| Duplicate key on re-import | `bash scripts/run_full_import.sh` truncates tables automatically |
| OOM during import | Lower `IMPORT_WORKERS`; enable guard: `GUARD_ENABLED=true` |
| Slow download | Normal for ~6 GB total; re-run skips completed files |

More detail: [IMPORT.md](IMPORT.md)
