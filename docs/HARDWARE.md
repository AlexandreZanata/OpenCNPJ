# Hardware Configuration Guide

Tune OpenCNPJ import and PostgreSQL for your machine.

> **Note:** Import is **CPU + disk + RAM** bound. The GPU (e.g. RTX 4060) is **not used** by the Go importer or PostgreSQL.

---

## Auto-detection

Scripts source `scripts/lib/hardware_profile.sh` and set defaults from installed RAM:

```bash
source scripts/lib/hardware_profile.sh
hardware_apply_env
# prints: Hardware profile: high-ram (32 GB RAM, 16 CPU cores)
```

Used by `download_and_import.sh` and `run_full_import.sh`.

---

## Profiles

### High RAM (≥ 28 GB) — e.g. 32 GB + RTX 4060

Recommended for full 215M-row import.

| Setting | Value | Why |
|---------|-------|-----|
| `IMPORT_WORKERS` | 8 | Matches parallel COPY without saturating RAM |
| `IMPORT_BATCH_SIZE` | 100,000 | Large COPY batches; good throughput |
| `GOMAXPROCS` | CPU cores | Use all cores for parser pool |
| `shared_buffers` (PG) | 16 GB | Already set in `docker-compose.yml` |
| `effective_cache_size` | 24 GB | OS page cache hint for planner |
| `maintenance_work_mem` | 10 GB | Faster index rebuild |
| `shm_size` | 4 GB | Docker shared memory for PG |

```bash
# docker-compose.yml is pre-tuned for 32 GB hosts
docker compose up -d postgres
make import-full
```

System guard defaults (`scripts/system_guard.conf`):

| Threshold | Value |
|-----------|-------|
| Memory warn | 25% available |
| Memory throttle | 18% available |
| Memory abort | 5% available |

### Mid RAM (14–27 GB)

```bash
export IMPORT_WORKERS=6
export IMPORT_BATCH_SIZE=75000
```

Reduce PostgreSQL `shared_buffers` in `docker-compose.yml` to **8 GB** and `effective_cache_size` to **12 GB**.

### Low RAM (< 14 GB)

```bash
export IMPORT_WORKERS=4
export IMPORT_BATCH_SIZE=50000
export GUARD_ENABLED=true
```

Use `make import-sample` (10%) instead of full import.

---

## Environment variables

| Variable | Default (32 GB) | Description |
|----------|-----------------|-------------|
| `IMPORT_WORKERS` | 8 | Parallel CSV file workers |
| `IMPORT_BATCH_SIZE` | 100000 | Rows per COPY batch |
| `GOMAXPROCS` | `nproc` | Go scheduler threads |
| `DATA_PATH` | `./data` | CSV directory |
| `IMPORT_MONITOR_INTERVAL` | 5 | Row-count poll seconds |
| `GUARD_ENABLED` | true (full import) | Abort before OOM |

---

## Disk requirements

| Item | Size |
|------|------|
| Raw CSV + ZIP (download) | ~6–8 GB |
| PostgreSQL data (100% import) | ~35–45 GB |
| WAL during import | ~10–20 GB peak |
| Indexes after rebuild | ~15–20 GB |
| **Recommended free** | **≥ 80 GB** |

Use SSD/NVMe. HDD works but import may take 2–3× longer.

---

## PostgreSQL Docker tuning

Current `docker-compose.yml` targets **32 GB RAM workstations**:

```yaml
shared_buffers=16GB
effective_cache_size=24GB
maintenance_work_mem=10GB
work_mem=256MB
max_parallel_workers=20
```

For 16 GB hosts, edit postgres `command:` block:

```yaml
- "shared_buffers=4GB"
- "effective_cache_size=12GB"
- "maintenance_work_mem=2GB"
```

Restart: `docker compose up -d postgres --force-recreate`

---

## Network (download bot)

| Factor | Tip |
|--------|-----|
| Bandwidth | RFB total ~6 GB compressed; plan 30–90 min on 100 Mbps |
| Retries | Default 3 attempts per file (`--retry=3`) |
| Resume | Re-run `make download` — completed files are skipped |
| Timeout | `--timeout=30` minutes per HTTP request |

---

## Measured performance

See [benchmarks/HARDWARE-RTX4060-32GB.md](benchmarks/HARDWARE-RTX4060-32GB.md) for import speeds on:

- **GPU:** NVIDIA RTX 4060 (8 GB VRAM) — not used by pipeline
- **RAM:** 32 GB
- **PostgreSQL:** 18.4-alpine (docker-compose defaults)

---

## Checklist before full import

- [ ] `df -h` shows ≥ 80 GB free
- [ ] `docker compose up -d postgres` healthy
- [ ] CSV files in `./data` (or run `make download` first)
- [ ] `go run ./cmd/migrate` applied
- [ ] `make guard-status` — memory OK
- [ ] Close memory-heavy apps (browsers, IDEs) during import
