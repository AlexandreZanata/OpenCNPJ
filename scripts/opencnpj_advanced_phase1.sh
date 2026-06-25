#!/usr/bin/env bash
# Plan 02 Phase 1 — VPS OS tuning gate (artifacts + optional live host checks).
# Usage: ./scripts/opencnpj_advanced_phase1.sh [API_BASE_URL]
# STRICT_VPS=1 enforces sysctl values and I/O scheduler on the host.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
API_BASE="${1:-http://localhost:8080}"
STRICT_VPS="${STRICT_VPS:-0}"
BENCH="$ROOT/.local/02-opencnpj-advanced-optimization/benchmarks"
K6_SCRIPT="$BENCH/k6-full.js"
SWAP_MAX_KIB=102400

pass=0
fail=0
skip=0

ok() { echo "[PASS] $*"; pass=$((pass + 1)); }
bad() { echo "[FAIL] $*"; fail=$((fail + 1)); }
warn() { echo "[SKIP] $*"; skip=$((skip + 1)); }

swap_used_kib() {
  free -k 2>/dev/null | awk '/^Swap:/ {print $3}' || echo 0
}

echo "=== OpenCNPJ advanced Phase 1 gate (VPS OS tuning) ==="
echo "API: $API_BASE  STRICT_VPS=$STRICT_VPS"
echo

echo "--- Delivery gate ---"
if go test ./internal/perfvalidation/... -short -run 'Phase1' >/dev/null 2>&1; then
  ok "go test internal/perfvalidation Phase1"
else
  bad "go test internal/perfvalidation Phase1"
fi

echo "--- Deploy artifacts ---"
for f in \
  deploy/vps/sysctl-opencnpj.conf \
  deploy/vps/limits-postgres.conf \
  deploy/vps/99-opencnpj-io-scheduler.rules \
  deploy/vps/fstab-postgres.example \
  deploy/vps/README.md \
  docs/ops/VPS-OS-TUNING.md; do
  if [[ -f "$ROOT/$f" ]]; then ok "artifact $f"; else bad "missing $f"; fi
done

sysctl_file="$ROOT/deploy/vps/sysctl-opencnpj.conf"
if grep -q 'vm.swappiness = 1' "$sysctl_file" && grep -q 'kernel.shmmax = 4294967296' "$sysctl_file"; then
  ok "sysctl template has swappiness + shmmax"
else
  bad "sysctl template incomplete"
fi

if ! grep -qiE '^[^#]*autovacuum|^[^#]*full_page_writes|^[^#]*wal_level' "$sysctl_file"; then
  ok "sysctl template excludes import-dev flags"
else
  bad "sysctl template contains forbidden import-dev flags"
fi

echo "--- Optional live host (STRICT_VPS=$STRICT_VPS) ---"
if [[ "$STRICT_VPS" == "1" ]]; then
  for kv in \
    'vm.swappiness=1' \
    'vm.dirty_ratio=10' \
    'vm.dirty_background_ratio=3' \
    'kernel.shmmax=4294967296' \
    'net.core.somaxconn=4096'; do
    key="${kv%%=*}"
    want="${kv#*=}"
    got=$(sysctl -n "$key" 2>/dev/null || echo "")
    if [[ "$got" == "$want" ]]; then ok "sysctl $key=$got"; else bad "sysctl $key=$got want $want"; fi
  done

  if command -v lsblk >/dev/null 2>&1; then
    ssd_count=$(lsblk -d -o ROTA 2>/dev/null | awk 'NR>1 && $1==0 {c++} END {print c+0}')
    if [[ "$ssd_count" -gt 0 ]]; then ok "lsblk reports $ssd_count non-rotational device(s)"; else warn "no SSD devices in lsblk"; fi
    if lsblk -d -o SCHED 2>/dev/null | grep -q 'mq-deadline'; then
      ok "mq-deadline scheduler present"
    else
      bad "mq-deadline scheduler not found (apply udev rules)"
    fi
  else
    warn "lsblk not available"
  fi
else
  warn "live sysctl/IO checks (set STRICT_VPS=1 on VPS after apply)"
fi

echo "--- Light load / swap stability ---"
swap_before=$(swap_used_kib)
k6_ok=0
K6_API="$API_BASE"
if [[ "$API_BASE" == http://localhost:* ]] || [[ "$API_BASE" == http://127.0.0.1:* ]]; then
  K6_API="${API_BASE/localhost/host.docker.internal}"
  K6_API="${K6_API/127.0.0.1/host.docker.internal}"
fi
if [[ -f "$K6_SCRIPT" ]] && command -v docker >/dev/null 2>&1; then
  for u in \
    "$API_BASE/api/v1/estabelecimentos/33000167000101" \
    "$API_BASE/api/v1/empresas/search?razao_social=PETROBRAS&limit=20" \
    "$API_BASE/api/v1/estabelecimentos/search?nome_fantasia=PADARIA&uf=SP&limit=20"; do
    curl -s -o /dev/null --max-time 15 "$u" || true
  done
  docker run --rm --add-host=host.docker.internal:host-gateway \
    -e API_BASE_URL="$K6_API" \
    -v "$K6_SCRIPT:/scripts/k6-full.js:ro" \
    grafana/k6 run --vus 3 --duration 15s /scripts/k6-full.js >/tmp/opencnpj-p1-k6.txt 2>&1 || true
  k6_fail_rate=$(grep 'http_req_failed' /tmp/opencnpj-p1-k6.txt 2>/dev/null | tail -1 | sed -n 's/.*: \([0-9.]*\)%.*/\1/p')
  if [[ -n "$k6_fail_rate" ]] && awk -v r="$k6_fail_rate" 'BEGIN{exit !(r+0 < 1)}'; then
    ok "light k6 completed (${k6_fail_rate}% errors)"
    k6_ok=1
  else
    warn "light k6 inconclusive (see /tmp/opencnpj-p1-k6.txt)"
    k6_ok=0
  fi
else
  for _ in 1 2 3 4 5; do
    curl -s -o /dev/null --max-time 10 \
      "$API_BASE/api/v1/estabelecimentos/search?nome_fantasia=PADARIA&uf=SP&limit=5" || true
  done
  ok "curl light load (k6 script or docker unavailable)"
  k6_ok=0
fi
swap_after=$(swap_used_kib)
swap_delta=$((swap_after - swap_before))
if [[ "$STRICT_VPS" == "1" && "${k6_ok:-0}" == "1" ]]; then
  if [[ "$swap_delta" -le "$SWAP_MAX_KIB" ]]; then
    ok "swap delta ${swap_delta} KiB <= ${SWAP_MAX_KIB} KiB"
  else
    bad "swap increased ${swap_delta} KiB (thrashing?)"
  fi
else
  warn "swap stability gate (STRICT_VPS=1 + successful k6 on VPS)"
fi

echo
echo "=== Summary: $pass passed, $fail failed, $skip skipped ==="
if [[ "$fail" -gt 0 ]]; then exit 1; fi

echo
echo "On VPS after apply: STRICT_VPS=1 $0 $API_BASE"
