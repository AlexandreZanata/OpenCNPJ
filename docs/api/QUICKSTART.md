# OpenCNPJ API — Quickstart

> Replace placeholders before running. Never commit real API keys.

## 1. Get an API key

Contact the platform admin or create one in the admin panel (`https://admin.comerc.app.br`).

Key format:

```
ocnpj_live_<32 hexadecimal characters>
```

## 2. Lookup a CNPJ

```bash
export API_KEY="ocnpj_live_YOUR_KEY_HERE"
export API_BASE="https://api.comerc.app.br"

curl -s -H "X-API-Key: ${API_KEY}" \
  "${API_BASE}/api/v1/cnpj/00000000000191" | jq .
```

## 3. Error handling

| HTTP | Meaning |
|------|---------|
| 401 | Invalid or missing `X-API-Key` |
| 404 | CNPJ not in database |
| 429 | Rate limit exceeded |
| 504 | Server timeout — retry with backoff |

## 4. Rate limits

Default: **60 requests/minute** per API key (configurable per client).

## 5. Full specification

See `docs/api/OPENAPI.yaml`.
