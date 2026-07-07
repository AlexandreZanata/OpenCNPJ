# Admin auth + TOTP MFA (Phase 5)

> Two-step login for `https://admin.comerc.app.br` — Argon2id password + mandatory TOTP.

## Routes

| Method | Path | Auth |
|--------|------|------|
| POST | `/admin/api/v1/auth/login` | None |
| POST | `/admin/api/v1/auth/mfa/verify` | Challenge ID |
| POST | `/admin/api/v1/auth/refresh` | HttpOnly refresh cookie |
| GET | `/admin/api/v1/me` | Bearer JWT (`mfaVerified: true`) |

## Environment

```bash
ADMIN_JWT_PRIVATE_KEY_PATH=/etc/opencnpj/jwt-private.pem
ADMIN_JWT_PUBLIC_KEY_PATH=/etc/opencnpj/jwt-public.pem
MFA_SECRET_ENCRYPTION_KEY=<32-byte-base64>
MFA_TOTP_ISSUER=OpenCNPJ-Admin          # config: saas.mfa_totp_issuer
REFRESH_TOKEN_COOKIE_NAME=opencnpj_admin_refresh
```

Generate RSA keys:

```bash
openssl genrsa -out jwt-private.pem 2048
openssl rsa -in jwt-private.pem -pubout -out jwt-public.pem
```

Generate AES-256 MFA encryption key:

```bash
openssl rand -base64 32
```

## Bootstrap (first deploy)

1. Run SaaS migrations (`cmd/migrate --saas`) — seeds placeholder admin row.
2. Provision password + TOTP:

```bash
go run ./cmd/admin-bootstrap --email YOUR_ADMIN_EMAIL
```

Scan the printed `otpauth://` URL once. Never commit password or TOTP secret.

## Config flags

```yaml
saas:
  admin_enabled: true
  admin_jwt_ttl_minutes: 15
  admin_refresh_ttl_days: 30
  mfa_required: true
  mfa_totp_issuer: "OpenCNPJ-Admin"
```

`saas.admin_enabled: true` requires Redis (MFA challenges + brute-force guard).

## Gate checklist

- Login without MFA code → `200` `{ "status": "mfa_required", ... }`
- Wrong TOTP → `401`
- Valid TOTP → JWT + HttpOnly refresh cookie
- `GET /admin/api/v1/me` without token → `401`

Automated gate: `go test ./internal/adminauth -run TestPhase5Gate -count=1`

## Related

- `docs/ops/SAAS-VPS-DEPLOY.md` — VPS layout
- `deploy/saas/api.env.example` — env template
- `migrations/saas/000004_admin_seed` — placeholder admin row
