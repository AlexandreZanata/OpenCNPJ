# Security Tooling Commands

Complete reference for security pipeline commands (CI and local).

---

## 1. Run the full stack locally

Runs in sequence: golangci-lint → gosec → staticcheck → govulncheck.  
Installs tools automatically if not in `PATH`.

```bash
./scripts/security-check.sh
```

**Requirement:** run from project root (where `go.mod` lives).  
**Exit code:** 0 if all passed, 1 if any check failed.

---

## 2. Tool installation (Go)

Each command installs to `$(go env GOPATH)/bin`. Ensure that directory is in `PATH`.

| Tool | Install command |
|------|-----------------|
| golangci-lint | `go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest` |
| gosec | `go install github.com/securego/gosec/v2/cmd/gosec@latest` |
| staticcheck | `go install honnef.co/go/tools/cmd/staticcheck@latest` |
| govulncheck | `go install golang.org/x/vuln/cmd/govulncheck@latest` |
| nancy | `go install github.com/sonatype-nexus-community/nancy@latest` |

**Example — add GOPATH/bin to PATH and install all:**

```bash
export PATH="${PATH}:$(go env GOPATH)/bin"

go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest
go install github.com/securego/gosec/v2/cmd/gosec@latest
go install honnef.co/go/tools/cmd/staticcheck@latest
go install golang.org/x/vuln/cmd/govulncheck@latest
go install github.com/sonatype-nexus-community/nancy@latest
```

---

## 3. Analysis commands (manual)

Run from **project root** (where `go.mod` and `.golangci.yml` live).

### 3.1 golangci-lint

Lint with gosec, staticcheck, errcheck, govet, ineffassign, unused (config in `.golangci.yml`).

```bash
golangci-lint run --timeout=5m --config=.golangci.yml ./...
```

### 3.2 gosec (SAST)

**Terminal output only:**

```bash
gosec ./...
```

**Generate SARIF (for GitHub Security upload):**

```bash
gosec -fmt sarif -out gosec-results.sarif ./...
```

### 3.3 staticcheck

```bash
staticcheck ./...
```

### 3.4 govulncheck (dependencies)

```bash
govulncheck ./...
```

**Verbose mode:**

```bash
govulncheck -show verbose ./...
```

### 3.5 nancy (dependencies — Sonatype OSS Index, optional)

OSS Index requires a free Sonatype account and API token. Not run in CI (use `govulncheck` there).

```bash
go list -json -m all | nancy sleuth --username "$OSS_INDEX_USERNAME" --token "$OSS_INDEX_TOKEN"
```

**Alternative using a file (if pipe is unsupported):**

```bash
go list -json -m all > go.list
nancy sleuth -p go.list
```

---

## 4. CI commands (GitHub Actions)

Workflow `.github/workflows/security.yml` runs the same commands in these jobs.

### Job: lint

- **Checkout:** `actions/checkout@v4`
- **Go:** `actions/setup-go@v5` with `go-version-file: go.mod` and `cache: true`
- **Lint:** `golangci-lint run --timeout=5m --config=.golangci.yml` (via `golangci/golangci-lint-action@v6`)

### Job: sast (depends on lint)

- **Install gosec:**  
  `go install github.com/securego/gosec/v2/cmd/gosec@latest`
- **Install staticcheck:**  
  `go install honnef.co/go/tools/cmd/staticcheck@latest`
- **Run gosec (SARIF):**  
  `gosec -fmt sarif -out gosec-results.sarif ./...`
- **Run staticcheck:**  
  `staticcheck ./...`
- **Upload SARIF:** `github/codeql-action/upload-sarif@v3` with `sarif_file: gosec-results.sarif` and `if: always()`

### Job: dependency-scan (depends on sast)

- **Install govulncheck:**  
  `go install golang.org/x/vuln/cmd/govulncheck@latest`
- **Run govulncheck:**  
  `govulncheck ./...`

---

## 5. One-liner summary (local)

If tools are already installed, repeat the four checks in script order:

```bash
golangci-lint run --timeout=5m --config=.golangci.yml ./... && \
gosec ./... && \
staticcheck ./... && \
govulncheck ./...
```

Optional nancy (requires OSS Index credentials):

```bash
go list -json -m all | nancy sleuth --username "$OSS_INDEX_USERNAME" --token "$OSS_INDEX_TOKEN"
```

---

## 6. Related configuration files

| File | Purpose |
|------|---------|
| `.golangci.yml` | golangci-lint config (linters, gosec severity/confidence, 5m timeout) |
| `.github/workflows/security.yml` | Security pipeline on GitHub Actions |
| `scripts/security-check.sh` | Local script: install (if needed) and run four checks |

See also: [SECURITY.md](SECURITY.md) for severity policy, suppressions, and vulnerability reporting.

---

## 7. Full code quality sequence

Recommended local validation order (matches CI):

```bash
# 0) Dependencies
go mod tidy

# 1) Formatting and imports
gofmt -w .
goimports -w .

# 2) Build and basic checks
go vet ./...
go test ./... -short -race -count=1

# 3) Full lint
golangci-lint run --timeout 5m

# 4) Security and static analysis
gosec ./...
staticcheck ./...
govulncheck ./...
# Optional: go list -json -m all | nancy sleuth --username "$OSS_INDEX_USERNAME" --token "$OSS_INDEX_TOKEN"

# 5) Integration tests
go test ./tests/integration/... -v -timeout 15m

# 6) Benchmarks
go test ./tests/benchmark/... -bench=. -benchmem -benchtime=5s -count=3
```

Makefile shortcuts:

```bash
make build
make lint
make test
make test-integration
make bench
make coverage
```

Conventional commit validation (local):

```bash
npm install --save-dev @commitlint/cli @commitlint/config-conventional
npx commitlint --from=HEAD~1 --to=HEAD --verbose
```
