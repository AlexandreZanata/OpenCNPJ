# Security Policy

This document describes security tooling, how to interpret results, severity policies, and how to report vulnerabilities.

---

## 1. Tools and what each detects

| Tool | Type | Finds | When it runs |
|------|------|-------|--------------|
| **gosec** | SAST | Hardcoded credentials, SQL injection, file permissions, weak crypto | PR, push, weekly |
| **staticcheck** | SAST | Bugs, deprecated APIs, performance, code correctness | PR, push, weekly |
| **govulncheck** | Dependency CVE | CVEs in Go modules (direct and indirect) | PR, push, weekly |
| **nancy** | Dependency CVE | Sonatype OSS Index (optional local; requires API token) | Local only |

- **SAST** (Static Application Security Testing): static analysis of source code.
- **Dependency CVE**: scans `go.mod` / `go.sum` against known vulnerability databases.

---

## 2. How to interpret results on GitHub

1. Open the repository on GitHub.
2. Go to the **Security** tab.
3. In the sidebar, click **Code scanning alerts** (or **Dependabot alerts** for dependencies).
4. gosec alerts appear under Code scanning (SARIF upload). Each finding includes:
   - **Severity**: Critical, High, Medium, Low.
   - **Rule**: e.g. G201 (SQL injection), G304 (path traversal).

**Severity meaning:**

- **Critical** / **High**: must be fixed before merge.
- **Medium**: blocks by default; may be suppressed with documented justification.
- **Low**: advisory only; does not block merge but should be reviewed.

On failure: fix the code or dependency, or (Medium only) suppress with a mandatory comment (see section 4).

---

## 3. Severity policy — when PRs are blocked

| Severity | Effect |
|----------|--------|
| **CRITICAL** | Merge blocked. |
| **HIGH** | Merge blocked. |
| **MEDIUM** | Blocked by default; suppressible with in-code justification. |
| **LOW** | Advisory only; merge allowed. |

The **Security** workflow fails if any job (lint, SAST, or dependency-scan) fails. A failed pipeline blocks merge until fixed or justified (Medium only).

---

## 4. Suppressing false positives

Every suppression must include a comment explaining why. Suppressions without comments are rejected in code review.

**gosec** — use `#nosec` with rule code and reason:

```go
// #nosec G201 -- false positive: input sanitized at L42
query := fmt.Sprintf("SELECT * FROM users WHERE id = %s", sanitizedID)
```

**staticcheck** (via golangci-lint) — use `//nolint`:

```go
//nolint:staticcheck -- brief reason (e.g. legacy API, migration in progress)
deprecatedAPICall()
```

**Rule:** every suppression comment must explain **why**. No comment = rejection in code review.

---

## 5. Reporting a vulnerability

- **Do not open a public issue** for security vulnerabilities.
- Email: **security@example.com**  
  *(Replace with the project or organization contact.)*
- Expected response time: **48 business hours**.

---

## 6. Weekly automated scan

- The **Security** workflow runs automatically every **Monday at 08:00 UTC** on `main`.
- Results appear in the repository **Security** tab (Code scanning and dependencies).
- Configure GitHub notifications under Settings → Notifications.

Run the same checks locally before push:

```bash
./scripts/security-check.sh
```

See [SECURITY-COMMANDS.md](SECURITY-COMMANDS.md) for full command reference.

---

## 8. SaaS production hardening (OpenCNPJ VPS)

When `saas.enabled: true` on the public API VPS (`api.comerc.app.br`), enforce the following.

### API layer

| Control | Implementation |
|---------|----------------|
| API key on all `/api/v1/*` customer routes | `X-API-Key` header via `internal/saas/middleware/api_key.go` |
| Constant-time key hash compare | `saas.SecureCompareKeyHash` (SHA-256 digest) |
| Masked API keys in logs | `saas.MaskAPIKey` via request logger |
| Request correlation | `X-Request-ID` middleware |
| Rate limits | Per-IP (`middleware.RateLimiter`) + per-client (Redis) |
| No debug in production | `saas.public_api_only: true` disables pprof |

### Admin layer

| Control | Implementation |
|---------|----------------|
| MFA mandatory | `saas.mfa_required` + login rejects admins without MFA |
| Argon2id passwords | 64 MB, t=3, p=4 (`internal/adminauth/password`) |
| Refresh token rotation | `usecase.Refresh` revokes old token on use |
| Secure cookies | HttpOnly, Secure (HTTPS), SameSite=Strict |
| CSRF on HTML forms | `_csrf` session token on all admin POST routes |
| Admin subdomain | `saas.admin_host` (e.g. `admin.comerc.app.br`) |

### Infrastructure (nginx / VPS)

- TLS 1.2+ only; HSTS `max-age=31536000; includeSubDomains`
- PostgreSQL `sslmode=require`; DB host IP whitelist
- Redis bound to `127.0.0.1` only
- `/metrics` — `metrics.internal_only` or `METRICS_BEARER_TOKEN`

### Secrets on disk

| Secret | Path (mode 600) |
|--------|-----------------|
| JWT private key | `/etc/opencnpj/jwt-private.pem` |
| MFA encryption key | `/etc/opencnpj/mfa.key` or `MFA_SECRET_ENCRYPTION_KEY` |
| DB passwords | `/etc/opencnpj/api.env` |

API key plaintext is **never** stored — only SHA-256 hash in `api_keys.key_hash`.

### Audit log

Admin actions are appended to `admin_audit_log`:

- client created / suspended
- API key created / revoked
- admin login success / failure
- MFA verification success

### Local gate

```bash
chmod +x scripts/security_hardening_gate.sh
./scripts/security_hardening_gate.sh
```

CI: `.github/workflows/security.yml` (gosec, staticcheck, govulncheck).

---

## 7. Data protection (LGPD)

CNPJ data may include masked CPF and partner names. Production deployments must:

- Enforce authentication and authorization before exposing search/export APIs
- Rate-limit public endpoints
- Log access without storing full CPF in plain text
- Follow LGPD/GDPR requirements for your jurisdiction
