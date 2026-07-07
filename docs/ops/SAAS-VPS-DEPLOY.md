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
| `deploy/saas/systemd-opencnpj-api.example` | systemd service |
| `deploy/saas/monthly-cnpj-sync.example.sh` | Monthly dump/upload/restore workflow |
| `config/config.saas.example.yaml` | Dual-database API config |
| `docs/api/OPENAPI.yaml` | Public API contract |

## Quick start (operator)

1. **Phase 0–1:** VPS inventory + nginx for `api.` / `admin.`
2. **Phase 2:** Create `opencnpj_cnpj` + `opencnpj_saas` on VPS Postgres.
3. **Phase 11:** First CNPJ dump from local PC → restore on VPS.
4. **Phase 3–4:** API keys + public CNPJ route.
5. **Phase 5–6:** Admin MFA + panel on `opencnpj_saas`.
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
- `docs/PERFORMANCE.md` — CNPJ lookup cache
