# Admin panel (Phase 6)

> Server-rendered HTML admin UI — no React, no Node on VPS.

## Stack

| Layer | Path |
|-------|------|
| Handlers | `internal/handlers/admin/` |
| Templates | `internal/templates/admin/` (embed.FS) |
| CSS | `internal/static/admin/admin.css` (< 8 KB) |
| Auth | Reuses Phase 5 MFA + JWT (`internal/adminauth/`) |

## Routes

| Method | Path | Action |
|--------|------|--------|
| GET | `/admin/login` | Login form |
| POST | `/admin/login` | Credentials → MFA challenge |
| GET | `/admin/mfa` | TOTP form |
| POST | `/admin/mfa` | Issue session + refresh cookie |
| POST | `/admin/logout` | Clear session |
| GET | `/admin/` | Dashboard (60s meta refresh) |
| GET | `/admin/clients` | Paginated client list (50/page) |
| GET/POST | `/admin/clients/new`, `/admin/clients` | Create client |
| GET | `/admin/clients/{id}` | Detail, keys, usage |
| POST | `/admin/clients/{id}/keys` | Generate key (one-time display) |
| POST | `/admin/clients/{id}/keys/{kid}/revoke` | Revoke key |
| POST | `/admin/clients/{id}/suspend` | Suspend client |
| GET | `/admin/usage` | Recent usage table |

Static assets: `GET /admin/static/admin.css`

External link: **API docs** → `saas.docs_public_url` (default GitHub `docs/api/QUICKSTART.md`)

## API documentation (customer)

| Doc | Path |
|-----|------|
| Quickstart | `docs/api/QUICKSTART.md` |
| OpenAPI | `docs/api/OPENAPI.yaml` |
| Errors | `docs/api/ERRORS.md` |
| Changelog | `docs/api/CHANGELOG.md` |

Optional Redoc UI: `GET /docs/` when `saas.docs_enabled: true` (off in production by default).

Gate: `./scripts/api_docs_gate.sh`

## Session

Fiber cookie session (`opencnpj_admin_session`) stores admin ID + access JWT after MFA.
Refresh token uses the same HttpOnly cookie as the JSON API (`REFRESH_TOKEN_COOKIE_NAME`).

## Enable

```yaml
saas:
  enabled: true
  admin_enabled: true
```

Requires Redis (MFA + brute-force) and admin JWT env vars — see [ADMIN-AUTH.md](ADMIN-AUTH.md).

## Gate checklist

| Check | Command / note |
|-------|----------------|
| HTML login → MFA → dashboard | `go test ./internal/handlers/admin -run TestPhase6Gate` |
| Create client + key in browser | Manual on staging VPS |
| Usage table after API call | Record via public API + check `/admin/usage` |
| No Node in production | No `web/` build on VPS |
| API RSS < 80 MB idle | `ps aux` on VPS after deploy |

## Related

- [ADMIN-AUTH.md](ADMIN-AUTH.md) — MFA, JWT, bootstrap CLI
- [SAAS-VPS-DEPLOY.md](SAAS-VPS-DEPLOY.md) — VPS layout
