# Dual-database VPS setup (Phase 2)

> Two PostgreSQL databases on the VPS: **`opencnpj_cnpj`** (read-only CNPJ) + **`opencnpj_saas`** (keys, admin, usage).

## Architecture

| Database | Writes from API | Updated |
|----------|-----------------|---------|
| `opencnpj_cnpj` | **Never** — SELECT only | Monthly `pg_dump` restore from local PC |
| `opencnpj_saas` | Yes — keys, usage, admin | Continuous |

**CNPJ import never runs on the VPS.** See [SAAS-VPS-DEPLOY.md](SAAS-VPS-DEPLOY.md) and `11-MONTHLY-CNPJ-SYNC.md` (local task plan).

## Repo templates

| File | Purpose |
|------|---------|
| `deploy/saas/postgres-bootstrap.sql.example` | Create DBs + roles + grants |
| `deploy/saas/pg_hba-snippet.example` | Localhost-only auth |
| `deploy/saas/api.env.example` | `CNPJ_DATABASE_URL`, `SAAS_DATABASE_URL`, `REDIS_URL` |
| `deploy/saas/pgbouncer.ini.example` | Optional connection pooling |
| `config/config.saas.example.yaml` | API dual-database config |

## VPS bootstrap (operator)

```bash
# 1. Postgres roles + databases
sudo -u postgres psql -v ON_ERROR_STOP=1 -f deploy/saas/postgres-bootstrap.sql.example
# Edit passwords in the file first — or set via ALTER ROLE after run

# 2. pg_hba — append deploy/saas/pg_hba-snippet.example, reload Postgres

# 3. Environment file
sudo mkdir -p /etc/opencnpj
sudo cp deploy/saas/api.env.example /etc/opencnpj/api.env
sudo chmod 600 /etc/opencnpj/api.env
# Edit URLs with real passwords

# 4. SaaS migrations (opencnpj_saas ONLY)
export SAAS_DATABASE_URL='postgres://opencnpj_saas:CHANGE_ME@127.0.0.1:5432/opencnpj_saas?sslmode=disable'
CONFIG_FILE=config/config.saas.example.yaml go run ./cmd/migrate --saas

# 5. First CNPJ dump from local PC → restore into opencnpj_cnpj (Phase 11)

# 6. Start API — /readyz pings both pools
export CNPJ_DATABASE_URL='postgres://opencnpj_reader:CHANGE_ME@127.0.0.1:5432/opencnpj_cnpj?sslmode=disable'
CONFIG_FILE=/etc/opencnpj/config.saas.yaml go run ./cmd/api
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8081/readyz   # 200
```

## Roles

| Role | Database | Purpose |
|------|----------|---------|
| `opencnpj_reader` | `opencnpj_cnpj` | API SELECT only |
| `opencnpj_saas` | `opencnpj_saas` | API read/write |
| `opencnpj_migrate_saas` | `opencnpj_saas` | Migrations only |
| `opencnpj_restore` | `opencnpj_cnpj` | Monthly dump restore only |

## SaaS migrations

| Migration | Scope |
|-----------|--------|
| `migrations/saas/000001_saas_metadata` | Tables: `api_clients`, `api_keys`, `admin_*`, usage |
| `migrations/saas/000002_saas_indexes` | Performance indexes for key lookup + client status |

**Never** run `go run ./cmd/migrate --saas` against `opencnpj_cnpj`.

CNPJ schema migrations (`000001`–`000016`) run on the **local import PC** before `pg_dump`.

## Validate (repo / CI)

```bash
./scripts/saas_dual_db_gate.sh           # template checks
./scripts/saas_dual_db_gate.sh --docker  # migrate + /readyz integration
go test ./deploy/saas/... -short
go test ./internal/database/... -short
```

## Phase 2 gate

```bash
psql "$CNPJ_DATABASE_URL" -c 'SELECT count(*) FROM estabelecimentos'   # after first dump
psql "$SAAS_DATABASE_URL" -c 'SELECT count(*) FROM api_clients'        # 0 or seeded
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8081/readyz   # 200
```

## Optional: pgBouncer

Point API URLs to `127.0.0.1:6432` when using `deploy/saas/pgbouncer.ini.example`.  
Two logical databases, `pool_mode = transaction`.

## Related

- `docs/ops/SAAS-VPS-DEPLOY.md` — full deploy guide
- `.local/03-saas-vps-comerc-api/02-DUAL-DATABASE.md` — task checklist
