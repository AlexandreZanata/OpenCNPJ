#!/usr/bin/env bash
# Mirror GitHub CI + Security workflows locally.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
export PATH="$(go env GOPATH)/bin:$PATH"

fail() { echo "FAIL: $*" >&2; exit 1; }
step() { echo ""; echo "==> $*"; }

step "commitlint (last push commits)"
if ! command -v commitlint >/dev/null 2>&1; then
  npx --no-install commitlint --version >/dev/null 2>&1 || npm install --no-save @commitlint/cli @commitlint/config-conventional
fi
BASE="${CI_BASE_SHA:-origin/main}"
if git rev-parse "$BASE" >/dev/null 2>&1; then
  git log "$BASE"..HEAD --format=%s | while read -r subject; do
    echo "$subject" | npx --no-install commitlint || fail "commitlint: $subject"
  done
else
  git log -1 --format=%s | npx --no-install commitlint
fi

step "golangci-lint"
command -v golangci-lint >/dev/null || go install github.com/golangci/golangci-lint/cmd/golangci-lint@v1.64.8
golangci-lint run --timeout=5m --config=.golangci.yml ./...

step "go test (unit, race)"
go test ./... -short -race -count=1

step "go vet"
go vet ./...

step "gosec"
command -v gosec >/dev/null || go install github.com/securego/gosec/v2/cmd/gosec@latest
gosec -fmt=text ./...

step "staticcheck"
command -v staticcheck >/dev/null || go install honnef.co/go/tools/cmd/staticcheck@latest
staticcheck ./...

step "go build ./cmd/..."
go build -v -ldflags="-s -w" ./cmd/...

step "govulncheck"
command -v govulncheck >/dev/null || go install golang.org/x/vuln/cmd/govulncheck@latest
govulncheck ./...

echo ""
echo "OK: local CI checks passed"
