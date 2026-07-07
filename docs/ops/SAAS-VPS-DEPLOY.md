# SaaS VPS deployment guide

> Public documentation for deploying OpenCNPJ as an API SaaS.  
> **No secrets in this repo** — use placeholders and server-side env files.

## Scope (v1)

- Domain: `comerc.app.br` (Cloudflare)
- API: `https://api.comerc.app.br/api/v1/cnpj/{cnpj}`
- Admin: `https://admin.comerc.app.br`
- Auth: per-client `X-API-Key`

## Two-database model

| Database | Where built | Where lives | Updated |
|----------|-------------|-------------|---------|
| **`opencnpj_cnpj`** | Local PC (import) | VPS Postgres | **Monthly** — `pg_dump` from PC → restore on VPS |
| **`opencnpj_saas`** | VPS migrations | VPS Postgres | Continuous — keys, admin, usage (never wiped by CNPJ sync) |

**The VPS never downloads or imports RFB data.** Your local workstation runs the full import pipeline; the VPS only receives a database dump.

## Monthly CNPJ update (operator)

1. **Local PC:** `./scripts/run_full_import.sh` (download + import + CNPJ migrations).
2. **Local PC:** `pg_dump -Fc` → compress → `rsync` to VPS.
3. **VPS:** Restore into `opencnpj_cnpj` (swap strategy) — see `deploy/saas/monthly-cnpj-sync.example.sh`.
4. **VPS:** Flush CNPJ cache in Redis; restart API. SaaS DB unchanged.

Full steps: `.local/03-saas-vps-comerc-api/11-MONTHLY-CNPJ-SYNC.md` (gitignored task plan).

## Local task plan

Detailed phased tasks: **`.local/03-saas-vps-comerc-api/TASKS.md`**

## Repo templates

| File | Purpose |
|------|---------|
| `deploy/saas/nginx-comerc.app.br.example` | Multi-site nginx vhost |
| `deploy/saas/cloudflare-real-ip.conf.example` | Cloudflare `set_real_ip_from` snippet |
| `deploy/saas/postgres-bootstrap.sql.example` | VPS Postgres DBs + roles |
| `deploy/saas/api.env.example` | `CNPJ_DATABASE_URL` + `SAAS_DATABASE_URL` |
| `deploy/saas/pgbouncer.ini.example` | Optional pgBouncer dual DB |
| `deploy/saas/systemd-opencnpj-api.example` | systemd service |
| `deploy/saas/monthly-cnpj-sync.example.sh` | Monthly dump/upload/restore workflow |
| `config/config.saas.example.yaml` | Dual-database API config |
| `docs/api/OPENAPI.yaml` | Public API contract |

## Quick start (operator)

1. **Phase 0–1:** VPS inventory + nginx for `api.` / `admin.` — see [NGINX-SAAS.md](NGINX-SAAS.md)
2. **Phase 2:** Create `opencnpj_cnpj` + `opencnpj_saas` on VPS Postgres — [DUAL-DATABASE-VPS.md](DUAL-DATABASE-VPS.md)
3. **Phase 11:** First CNPJ dump from local PC → restore on VPS.
4. **Phase 3–4:** API keys + public CNPJ route.
5. **Phase 5:** Admin MFA — see [ADMIN-AUTH.md](ADMIN-AUTH.md)
6. **Phase 6:** Admin panel on `opencnpj_saas`.
6. **Monthly:** Repeat Phase 11 only for CNPJ data refresh.

### Apply SaaS migrations (VPS)

```bash
export SAAS_DATABASE_URL='postgres://opencnpj_saas:CHANGE_ME@127.0.0.1:5432/opencnpj_saas?sslmode=disable'
CONFIG_FILE=/etc/opencnpj/config.saas.yaml go run ./cmd/migrate --saas
```

CNPJ schema migrations (`000001`–`000014`) run on the **local import PC** only. Never run `--saas` against `opencnpj_cnpj`.

### Start API (dual database)

```bash
export CNPJ_DATABASE_URL='postgres://opencnpj_reader:CHANGE_ME@127.0.0.1:5432/opencnpj_cnpj?sslmode=disable'
export SAAS_DATABASE_URL='postgres://opencnpj_saas:CHANGE_ME@127.0.0.1:5432/opencnpj_saas?sslmode=disable'
CONFIG_FILE=/etc/opencnpj/config.saas.yaml go run ./cmd/api
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8081/readyz   # expect 200
```

When `saas.enabled: true`, `/readyz` pings **both** PostgreSQL pools. With `saas.public_api_only: true`, only `GET /api/v1/cnpj/:cnpj` is registered (plus health/metrics).

### Dual-database gate (Phase 2)

```bash
./scripts/saas_dual_db_gate.sh           # template checks
./scripts/saas_dual_db_gate.sh --docker  # migrate + /readyz (CI/local)
```

See [DUAL-DATABASE-VPS.md](DUAL-DATABASE-VPS.md) for VPS operator steps.

### API keys + usage (Phase 3)

When `saas.enabled: true`, all `/api/v1/*` routes require `X-API-Key` (`ocnpj_live_<32 hex>`).

| Component | Path |
|-----------|------|
| sqlc queries | `db/queries/saas/` |
| Generated code | `internal/db/saas/` |
| Domain logic | `internal/saas/` |
| Middleware | `internal/saas/middleware/api_key.go` |

```bash
make sqlc                                    # regenerate after SQL changes
./scripts/saas_api_key_gate.sh               # unit gate
./scripts/saas_api_key_gate.sh --docker      # + Postgres EXPLAIN integration
```

Seed a test client (SQL on `opencnpj_saas`):

```sql
-- Use internal/saas.CreateClientKey from a one-off admin CLI, or insert via admin panel (Phase 6).
```

Manual gate:

```bash
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8081/api/v1/cnpj/00000000000191          # 401
curl -s -o /dev/null -w '%{http_code}\n' -H "X-API-Key: ocnpj_live_..." \
  http://127.0.0.1:8081/api/v1/cnpj/00000000000191   # 200 or 404
```

### Public CNPJ API (Phase 4)

`GET /api/v1/cnpj/:cnpj` uses sqlc + pgx (`CNPJPool`) with parallel fetch via `errgroup`, L1/Redis cache, and a slim public DTO (no internal UUIDs).

| Component | Path |
|-----------|------|
| sqlc queries | `db/queries/cnpj/` |
| Schema snapshot | `db/schema/cnpj.sql` |
| Lookup service | `internal/cnpj/` |
| Handler | `internal/handlers/cnpj_handler.go` |

```bash
./scripts/saas_public_cnpj_gate.sh               # unit gate
./scripts/saas_public_cnpj_gate.sh --docker      # + EXPLAIN idx_estabelecimentos_cnpj_completo
```

With `saas.public_api_only: true`, only `GET /api/v1/cnpj/:cnpj` is registered under `/api/v1` (plus health/metrics). Cache headers: `Cache-Control: private, max-age=300`.

### Nginx (Phase 1)

```bash
./scripts/validate_nginx_saas.sh   # local syntax + directive gate
# On VPS: copy templates from deploy/saas/ — see docs/ops/NGINX-SAAS.md
curl -sI https://api.comerc.app.br/readyz   # 200 or 502, not 404
```

## Memory budget

| Component | VPS RAM target |
|-----------|----------------|
| OpenCNPJ API + admin | ≤ 512 MB |
| Redis | ≤ 128 MB |
| Postgres | bulk of RAM → `opencnpj_cnpj` |

## Security

- CNPJ DB role: `SELECT` only
- SaaS DB: separate credentials
- API keys: SHA-256 hash only
- Admin: Argon2id + mandatory TOTP

## Related docs

- `docs/IMPORT.md` — local import pipeline
- `docs/ops/VPS-POSTGRESQL.md` — Postgres tuning on VPS
- `docs/ops/NGINX-SAAS.md` — nginx + Cloudflare for api/admin subdomains
- `docs/ops/DUAL-DATABASE-VPS.md` — two Postgres databases on VPS
- `docs/PERFORMANCE.md` — CNPJ lookup cache
