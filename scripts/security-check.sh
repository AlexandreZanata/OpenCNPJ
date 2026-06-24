#!/usr/bin/env bash
set -euo pipefail

# Run the security stack locally before push: golangci-lint, gosec, staticcheck, govulncheck.
# Usage: ./scripts/security-check.sh

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

PASSED=0
FAILED=0

# Ensure GOPATH/bin is on PATH for tools installed via go install
export PATH="${PATH}:$(go env GOPATH)/bin"

run_check() {
  local name="$1"
  local cmd="$2"
  echo -e "${YELLOW}▶ Running: ${name}${NC}"
  if eval "$cmd"; then
    echo -e "${GREEN}✓ ${name} passed${NC}"
    PASSED=$((PASSED + 1))
  else
    echo -e "${RED}✗ ${name} failed${NC}"
    FAILED=$((FAILED + 1))
  fi
}

# Verify/install tools
ensure_tool() {
  local name="$1"
  local install_cmd="$2"
  if ! command -v "$name" &>/dev/null; then
    echo -e "${YELLOW}Installing ${name}...${NC}"
    eval "$install_cmd"
  fi
}

ensure_tool golangci-lint 'go install github.com/golangci/golangci-lint/cmd/golangci-lint@latest'
ensure_tool gosec 'go install github.com/securego/gosec/v2/cmd/gosec@latest'
ensure_tool staticcheck 'go install honnef.co/go/tools/cmd/staticcheck@latest'
ensure_tool govulncheck 'go install golang.org/x/vuln/cmd/govulncheck@latest'

echo ""
echo "========== Security checks =========="
echo ""

run_check "golangci-lint" "golangci-lint run --timeout=5m --config=.golangci.yml ./..."
run_check "gosec" "gosec ./..."
run_check "staticcheck" "staticcheck ./..."
run_check "govulncheck" "govulncheck ./..."

echo ""
echo "========== Summary =========="
echo -e "Passed: ${GREEN}${PASSED}${NC}"
echo -e "Failed: ${RED}${FAILED}${NC}"
echo ""

if [ "$FAILED" -gt 0 ]; then
  exit 1
fi
exit 0
