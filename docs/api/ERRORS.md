# OpenCNPJ API — Error responses

All error responses use JSON:

```json
{
  "error": "machine_readable_code",
  "message": "Human-readable detail",
  "code": 401
}
```

| Field | Type | Description |
|-------|------|-------------|
| `error` | string | Stable identifier for client branching |
| `message` | string | Short explanation (may change) |
| `code` | integer | HTTP status code (duplicate of response status) |

## Authentication and client status

| HTTP | `error` | When |
|------|---------|------|
| 401 | `missing_api_key` | `X-API-Key` header absent |
| 401 | `invalid_api_key` | Key not found or wrong hash |
| 401 | `expired_api_key` | Key past `expires_at` |
| 403 | `client_suspended` | Client account suspended |

## Rate limits and quota

| HTTP | `error` | When |
|------|---------|------|
| 429 | `rate_limit_exceeded` | Per-key requests/minute exceeded |
| 429 | `quota_exceeded` | Monthly lookup quota exceeded |

## CNPJ lookup (`GET /api/v1/cnpj/{cnpj}`)

| HTTP | `error` | When |
|------|---------|------|
| 400 | `invalid_cnpj` | Not 14 digits or check digits invalid |
| 404 | `cnpj_not_found` | Valid CNPJ, not in database |
| 500 | `internal_error` | Unexpected server failure |
| 504 | — | Query timeout (Fiber timeout middleware; no JSON body guaranteed) |

## Examples

### Missing API key

```http
HTTP/1.1 401 Unauthorized
Content-Type: application/json

{"error":"missing_api_key","message":"missing api key","code":401}
```

### Invalid CNPJ

```http
HTTP/1.1 400 Bad Request

{"error":"invalid_cnpj","message":"invalid cnpj","code":400}
```

### Not found

```http
HTTP/1.1 404 Not Found

{"error":"cnpj_not_found","message":"cnpj not found","code":404}
```

## Client guidance

- Retry **429** and **504** with exponential backoff.
- Do **not** retry **400**, **401**, **403**, or **404**.
- Log `error` field, not raw `message`, for alerting rules.

See also: `docs/api/OPENAPI.yaml`, `docs/api/QUICKSTART.md`.
