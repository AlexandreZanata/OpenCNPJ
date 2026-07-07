# SaaS nginx + Cloudflare (multi-site VPS)

> Phase 1 — reverse proxy for `api.` and `admin.` without touching existing vhosts.

## Cloudflare DNS

| Type | Name | Content | Proxy |
|------|------|---------|-------|
| A | `api` | `YOUR_VPS_IP` | Proxied |
| A | `admin` | `YOUR_VPS_IP` | Proxied |

**SSL/TLS mode:** Full (strict). Origin certificates via Let's Encrypt on the VPS.

### Optional edge rate limit

Cloudflare dashboard → Security → WAF → Rate limiting rule:

- Expression: `(http.host eq "api.comerc.app.br")`
- Requests: e.g. 100 per minute per IP (tune per plan)

**Caution:** Bot Fight Mode may block legitimate API clients — test before enabling.

## Nginx install (additive only)

```bash
sudo cp deploy/saas/cloudflare-real-ip.conf.example /etc/nginx/snippets/cloudflare-real-ip.conf
sudo cp deploy/saas/nginx-comerc.app.br.example /etc/nginx/sites-available/opencnpj-comerc
sudo ln -sf /etc/nginx/sites-available/opencnpj-comerc /etc/nginx/sites-enabled/
sudo certbot certonly --nginx -d api.comerc.app.br -d admin.comerc.app.br
sudo nginx -t && sudo systemctl reload nginx
```

**Do not** remove or overwrite configs for the other 3 sites on the VPS.

## Key settings

| Setting | Value | Why |
|---------|-------|-----|
| `limit_req_zone` | 10 r/s per IP | DDoS cushion before app |
| `location = /readyz` | no rate limit | Health probes / Cloudflare checks |
| `proxy_read_timeout` | 30s (API routes) | CNPJ lookup max |
| `client_max_body_size` | 1m | v1 has no upload |
| `upstream opencnpj_api` | `127.0.0.1:8081` | Isolated from other apps |
| `/metrics` | `deny all` | Prometheus internal only |

## Real client IP behind Cloudflare

Snippet: `deploy/saas/cloudflare-real-ip.conf.example`  
Refresh ranges periodically from [Cloudflare IP list](https://www.cloudflare.com/ips/).

## Validate (repo / CI)

```bash
./scripts/validate_nginx_saas.sh
go test ./deploy/saas/... -short
```

## Verify on VPS

```bash
# After reload — existing sites must still respond
curl -sI https://api.comerc.app.br/readyz
# 200 (API up) or 502 (API down) — NOT 404 (wrong vhost)

curl -sI https://admin.comerc.app.br/readyz
```

## Templates

| File | Purpose |
|------|---------|
| `deploy/saas/nginx-comerc.app.br.example` | API + admin vhosts |
| `deploy/saas/cloudflare-real-ip.conf.example` | `set_real_ip_from` snippet |

## Related

- `docs/ops/SAAS-VPS-DEPLOY.md` — full deploy guide
- `.local/03-saas-vps-comerc-api/01-INFRA-VPS-NGINX.md` — task checklist
