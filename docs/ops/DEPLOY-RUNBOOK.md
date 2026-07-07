# Production deploy runbook (Phase 10)

> Operator guide: empty VPS → working `GET /api/v1/cnpj/{cnpj}` on `api.comerc.app.br`.  
> **No secrets in this repo** — generate on the VPS only.

## Pre-deploy checklist

- [ ] Phases 0–9 complete (or 0–4 for API-only v1 without admin UI)
- [ ] `opencnpj_saas` migrated (`migrations/saas/000001`–`000004`)
- [ ] First `opencnpj_cnpj` restore from local PC dump (Phase 11)
- [ ] DNS `api.comerc.app.br` and `admin.comerc.app.br` → VPS
- [ ] nginx config tested (`./scripts/validate_nginx_saas.sh`)
- [ ] Secrets generated on VPS (`/etc/opencnpj/`, mode 600/700)

## 1. Clone and build

```bash
sudo useradd -r -m -d /opt/opencnpj -s /bin/bash opencnpj || true
sudo mkdir -p /opt/opencnpj
sudo chown opencnpj:opencnpj /opt/opencnpj
cd /opt/opencnpj
git clone https://github.com/YOUR_ORG/BUSCA-CNPJ-2026.git .
./scripts/build_opencnpj_api.sh /usr/local/bin/opencnpj-api
sudo cp /usr/local/bin/opencnpj-api /usr/local/bin/opencnpj-api.bak
```

## 2. Config and secrets

```bash
sudo mkdir -p /etc/opencnpj /var/log/opencnpj
sudo chmod 700 /etc/opencnpj
sudo cp config/config.saas.example.yaml /etc/opencnpj/config.saas.yaml
sudo cp deploy/saas/api.env.example /etc/opencnpj/api.env
sudo chmod 600 /etc/opencnpj/api.env
sudo nano /etc/opencnpj/api.env   # fill CHANGE_ME placeholders
```

Required env keys (see `deploy/saas/api.env.example`):

| Variable | Purpose |
|----------|---------|
| `CONFIG_FILE` | Path to `config.saas.yaml` |
| `CNPJ_DATABASE_URL` | Read-only CNPJ pool |
| `SAAS_DATABASE_URL` | Keys, admin, usage |
| `REDIS_URL` | Rate limits + MFA |
| `ADMIN_JWT_PRIVATE_KEY_PATH` | RS256 signing |
| `MFA_SECRET_ENCRYPTION_KEY` | TOTP secret encryption |

## 3. PostgreSQL bootstrap

```bash
sudo -u postgres psql -f deploy/saas/postgres-bootstrap.sql.example
export SAAS_DATABASE_URL='postgres://opencnpj_saas:...@127.0.0.1:5432/opencnpj_saas?sslmode=disable'
CONFIG_FILE=/etc/opencnpj/config.saas.yaml go run ./cmd/migrate --saas
```

## 4. Redis (dedicated instance, low memory)

```bash
sudo cp deploy/saas/redis-opencnpj.conf.example /etc/redis/opencnpj.conf
redis-server /etc/redis/opencnpj.conf --daemonize yes
redis-cli -p 6381 ping   # PONG
```

Or one-liner (staging only):

```bash
redis-server --port 6381 --bind 127.0.0.1 --maxmemory 128mb \
  --maxmemory-policy allkeys-lru --daemonize yes
```

## 5. nginx

```bash
sudo cp deploy/saas/cloudflare-real-ip.conf.example /etc/nginx/snippets/cloudflare-real-ip.conf
sudo cp deploy/saas/nginx-comerc.app.br.example /etc/nginx/sites-available/opencnpj-comerc
sudo ln -sf /etc/nginx/sites-available/opencnpj-comerc /etc/nginx/sites-enabled/
sudo certbot certonly --nginx -d api.comerc.app.br -d admin.comerc.app.br
sudo nginx -t && sudo systemctl reload nginx
```

## 6. systemd

```bash
sudo cp deploy/saas/systemd-opencnpj-api.example /etc/systemd/system/opencnpj-api.service
sudo systemctl daemon-reload
sudo systemctl enable --now opencnpj-api
sudo systemctl status opencnpj-api
```

## 7. Bootstrap admin (first deploy only)

```bash
sudo -u opencnpj env $(grep -v '^#' /etc/opencnpj/api.env | xargs) \
  go run ./cmd/admin-bootstrap --email admin@example.com
# Scan TOTP QR / save recovery codes offline
```

## 8. Smoke test

```bash
# Health only (no API key)
./scripts/saas_smoke.sh https://api.comerc.app.br

# Full path including authenticated CNPJ lookup
export TEST_API_KEY='ocnpj_live_...'
./scripts/saas_smoke.sh https://api.comerc.app.br "$TEST_API_KEY"
```

Local (before DNS):

```bash
./scripts/saas_smoke.sh http://127.0.0.1:8081
```

## 9. Rollback

```bash
sudo ./deploy/saas/rollback.example.sh
```

Or manually:

```bash
sudo systemctl stop opencnpj-api
sudo cp /usr/local/bin/opencnpj-api.bak /usr/local/bin/opencnpj-api
sudo systemctl start opencnpj-api
./scripts/saas_smoke.sh http://127.0.0.1:8081
```

## Monitoring

| Check | Command / target |
|-------|------------------|
| Uptime | `curl -sf https://api.comerc.app.br/readyz` (cron every 60s) |
| Liveness | `curl -sf https://api.comerc.app.br/livez` |
| Prometheus | Scrape `127.0.0.1:8081/metrics` via VPN or SSH tunnel |
| Alerts | p95 > 500 ms, 5xx rate > 1%, Redis down |

## Post-deploy

- [ ] Create first production client + API key via admin panel
- [ ] Deliver customer key over secure channel (never email/Slack plaintext)
- [ ] Record deploy date in server log (`/var/log/opencnpj/deploy.log`, not in git)

## Local gate (CI / pre-push)

```bash
./scripts/saas_deploy_gate.sh           # template + unit checks
./scripts/saas_deploy_gate.sh --docker  # migrate + API + smoke
```

## Related docs

- [SAAS-VPS-DEPLOY.md](SAAS-VPS-DEPLOY.md) — overview and phase index
- [DUAL-DATABASE-VPS.md](DUAL-DATABASE-VPS.md) — two-database model
- [NGINX-SAAS.md](NGINX-SAAS.md) — Cloudflare + TLS
- [SECURITY.md](../SECURITY.md) §8 — SaaS hardening
