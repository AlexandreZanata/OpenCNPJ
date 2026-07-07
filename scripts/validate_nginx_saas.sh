#!/usr/bin/env bash
# Validate SaaS nginx templates (syntax + required directives).
# Usage: ./scripts/validate_nginx_saas.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TEMPLATE="$ROOT/deploy/saas/nginx-comerc.app.br.example"
CF_SNIPPET="$ROOT/deploy/saas/cloudflare-real-ip.conf.example"

required=(
  'limit_req_zone $binary_remote_addr zone=opencnpj_api'
  'upstream opencnpj_api'
  '127.0.0.1:8081'
  'server_name api.comerc.app.br'
  'server_name admin.comerc.app.br'
  'client_max_body_size 1m'
  'location = /readyz'
  'proxy_read_timeout 30s'
  'include /etc/nginx/snippets/cloudflare-real-ip.conf'
)

echo "==> Checking required directives in nginx template"
for needle in "${required[@]}"; do
  if ! grep -qF "$needle" "$TEMPLATE"; then
    echo "MISSING: $needle" >&2
    exit 1
  fi
done

echo "==> Checking Cloudflare real-IP snippet"
if ! grep -qF 'real_ip_header CF-Connecting-IP' "$CF_SNIPPET"; then
  echo "Cloudflare snippet missing real_ip_header" >&2
  exit 1
fi
if ! grep -qF 'set_real_ip_from 173.245.48.0/20' "$CF_SNIPPET"; then
  echo "Cloudflare snippet missing IPv4 ranges" >&2
  exit 1
fi

if command -v docker >/dev/null 2>&1; then
  echo "==> nginx -t via Docker (syntax)"
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' EXIT

  mkdir -p "$work/snippets" "$work/certs" "$work/conf.d"
  cp "$TEMPLATE" "$work/conf.d/opencnpj-comerc.conf"
  cp "$CF_SNIPPET" "$work/snippets/cloudflare-real-ip.conf"

  openssl req -x509 -nodes -newkey rsa:2048 -days 1 \
    -keyout "$work/certs/privkey.pem" \
    -out "$work/certs/fullchain.pem" \
    -subj "/CN=api.comerc.app.br" 2>/dev/null

  sed -i \
    -e "s|/etc/letsencrypt/live/api.comerc.app.br/fullchain.pem|/etc/nginx/certs/fullchain.pem|g" \
    -e "s|/etc/letsencrypt/live/api.comerc.app.br/privkey.pem|/etc/nginx/certs/privkey.pem|g" \
    -e "s|/etc/letsencrypt/live/admin.comerc.app.br/fullchain.pem|/etc/nginx/certs/fullchain.pem|g" \
    -e "s|/etc/letsencrypt/live/admin.comerc.app.br/privkey.pem|/etc/nginx/certs/privkey.pem|g" \
    "$work/conf.d/opencnpj-comerc.conf"

  docker run --rm \
    -v "$work/conf.d:/etc/nginx/conf.d:ro" \
    -v "$work/snippets:/etc/nginx/snippets:ro" \
    -v "$work/certs:/etc/nginx/certs:ro" \
    nginx:1.27-alpine \
    nginx -t

  echo "nginx -t OK"
else
  echo "SKIP: docker not available — directive checks only"
fi

echo "OK: SaaS nginx templates valid"
