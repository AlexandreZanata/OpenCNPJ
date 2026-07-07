# OpenCNPJ API — Quickstart

> Replace placeholders before running. **Never commit real API keys.**

## 1. Get an API key

Create a client and key in the admin panel (`https://admin.comerc.app.br`) or ask your platform operator.

Key format:

```
ocnpj_live_<32 hexadecimal characters>
```

## 2. Lookup a CNPJ

```bash
export API_KEY="ocnpj_live_YOUR_KEY_HERE"
export API_BASE="https://api.comerc.app.br"

curl -sS -H "X-API-Key: ${API_KEY}" \
  "${API_BASE}/api/v1/cnpj/00000000000191" | jq .
```

### Local development

```bash
export API_BASE="http://localhost:8080"
# Enable docs UI: saas.docs_enabled: true in config (off in production by default)
open "${API_BASE}/docs"
```

## 3. Error handling

See [ERRORS.md](ERRORS.md) for the full catalog.

| HTTP | `error` (typical) | Action |
|------|-------------------|--------|
| 400 | `invalid_cnpj` | Fix request |
| 401 | `missing_api_key` / `invalid_api_key` | Check header |
| 403 | `client_suspended` | Contact operator |
| 404 | `cnpj_not_found` | Not in database |
| 429 | `rate_limit_exceeded` / `quota_exceeded` | Back off |
| 504 | — | Retry with backoff |

## 4. Rate limits

Default: **60 requests/minute** per API key (configurable per client). Optional monthly quota when set on the client.

## 5. Specification

| Document | Purpose |
|----------|---------|
| [OPENAPI.yaml](OPENAPI.yaml) | OpenAPI 3.1 contract |
| [ERRORS.md](ERRORS.md) | Error codes |
| [CHANGELOG.md](CHANGELOG.md) | Version history |

Validate locally:

```bash
./scripts/api_docs_gate.sh
```
