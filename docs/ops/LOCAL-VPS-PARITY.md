# Local VPS API parity

Run the **same API + PostgreSQL production profile** as the 16 GB VPS on your workstation for frontend speed tests.

## Strategy

| Phase | Postgres profile | Purpose |
|-------|------------------|---------|
| Import | `docker-compose.yml` (fast COPY flags) | Clean full import in ~20–40 min |
| API tests | `docker-compose.vps-parity.yml` | VPS GUCs: 4 GB `shared_buffers`, 64 MB `work_mem`, autovacuum ON |

API runtime: `config/config.vps-parity.yaml` — L1 cache ON, rate limiter ON (`BENCHMARK_MODE` unset).

## One command (clean import + benchmark)

```bash
./scripts/local_vps_parity_stack.sh
```

## Manual steps

```bash
# 1. Clean import (fast PG)
docker compose down -v
docker compose up -d postgres pgbouncer redis
go run ./cmd/migrate
bash scripts/run_full_import.sh

# 2. VPS production Postgres
docker compose -f docker-compose.yml -f docker-compose.vps-parity.yml up -d postgres --force-recreate
bash scripts/vps_analyze_search_tables.sh
bash scripts/refresh_stats_aggregates.sh

# 3. API + web
CONFIG_FILE=config/config.vps-parity.yaml go run ./cmd/api   # terminal 1
make web-dev                                                  # terminal 2

# 4. Benchmark
./scripts/local_vps_frontend_benchmark.sh http://localhost:8080
```

Report: `docs/benchmarks/YYYY-MM-DD-vps-parity-local-frontend.md`

## Reuse existing data (skip import)

```bash
SKIP_IMPORT=1 CLEAN=0 ./scripts/local_vps_parity_stack.sh
```

## VPS reference

- `deploy/vps/postgresql-opencnpj.conf.example`
- `docs/ops/VPS-POSTGRESQL.md`
