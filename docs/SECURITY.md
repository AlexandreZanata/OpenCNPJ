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

## 7. Data protection (LGPD)

CNPJ data may include masked CPF and partner names. Production deployments must:

- Enforce authentication and authorization before exposing search/export APIs
- Rate-limit public endpoints
- Log access without storing full CPF in plain text
- Follow LGPD/GDPR requirements for your jurisdiction
