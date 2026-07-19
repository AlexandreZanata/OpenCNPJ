#!/usr/bin/env bash
# After redownload_truncated_rfb.sh finishes: verify CSVs, full import, dump, upload VPS.
#
# Usage:
#   ./scripts/repair_truncated_pipeline.sh              # wait for download log, then import+sync
#   ./scripts/repair_truncated_pipeline.sh --import-only # skip wait/upload
#   ./scripts/repair_truncated_pipeline.sh --sync-only   # dump+upload (DB already imported)
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

export PATH="${HOME}/.local/go/bin:${HOME}/go/bin:/usr/local/go/bin:${PATH}"

DATA="${DATA_PATH:-$ROOT/data}"
LOG="${REDOWNLOAD_LOG:-/tmp/redownload_truncated_rfb.log}"
LIMIT=$((512 * 1024 * 1024))
MIN_ESTAB="${MIN_ESTABELECIMENTOS:-40000000}"
DUMP_TAG="${DUMP_TAG:-$(date +%Y%m)}"
MODE="full"

for arg in "$@"; do
  case "$arg" in
    --import-only) MODE="import" ;;
    --sync-only) MODE="sync" ;;
    -h|--help) sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "Unknown flag: $arg" >&2; exit 1 ;;
  esac
done

log() { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }

downloader_running() {
  pgrep -f '[/]cmd/downloader|[g]o-build/.*/downloader' >/dev/null 2>&1
}

wait_download() {
  log "Waiting for downloader to finish (log: $LOG)"
  while downloader_running; do
    local line
    line="$(tr '\r' '\n' <"$LOG" 2>/dev/null | grep -E 'download|\[ok\]|done:|error|truncated' | tail -1 || true)"
    log "download running | ${line:-…}"
    sleep 30
  done
  if grep -qiE 'truncated extract|zip member exceeds|FAIL|fatal' "$LOG" 2>/dev/null; then
    log "ERROR: download log reports failure — inspect $LOG" >&2
    exit 1
  fi
  if ! grep -q 'Download complete' "$LOG" 2>/dev/null; then
    log "WARN: no 'Download complete' line; verifying CSV sizes anyway"
  fi
}

assert_no_truncated_csvs() {
  local bad=0
  while IFS= read -r -d '' f; do
    log "ERROR: still truncated: $f"
    bad=1
  done < <(find "$DATA" -maxdepth 1 -type f \( \
    -name '*.ESTABELE' -o -name '*SOCIOCSV' -o -name '*SIMPLES*' \
    \) -size "${LIMIT}c" -print0 2>/dev/null)
  local n_estab
  n_estab=$(find "$DATA" -maxdepth 1 -name '*.ESTABELE' | wc -l)
  if [[ "$n_estab" -lt 10 ]]; then
    log "ERROR: expected 10 ESTABELE files, found $n_estab" >&2
    exit 1
  fi
  if [[ "$bad" -ne 0 ]]; then
    exit 1
  fi
  log "OK: no 512 MiB truncated ESTABELE/SOCIO/SIMPLES CSVs ($n_estab ESTABELE files)"
  find "$DATA" -maxdepth 1 -name '*.ESTABELE' -printf '%s %f\n' | sort -k2 | while read -r sz name; do
    log "  $name $(numfmt --to=iec-i --suffix=B "$sz" 2>/dev/null || echo "${sz}B")"
  done
}

run_import() {
  log "Starting full import (MIN_ESTABELECIMENTOS=$MIN_ESTAB)"
  # Agent shells often lack docker.sock group — re-exec under sg docker.
  if ! docker info >/dev/null 2>&1; then
    if groups | grep -qw docker; then
      log "Re-exec import under sg docker"
      sg docker -c "cd '$ROOT' && bash '$ROOT/scripts/run_full_import.sh'"
      local estab
      estab=$(sg docker -c "docker exec receita-postgres psql -U receita_user -d receita_db -At -c 'SELECT count(*) FROM estabelecimentos'" | tr -d '[:space:]')
      log "estabelecimentos=$estab"
      if [[ -z "$estab" || "$estab" -lt "$MIN_ESTAB" ]]; then
        log "ERROR: estabelecimentos=$estab below $MIN_ESTAB — import incomplete" >&2
        exit 1
      fi
      return 0
    fi
    log "ERROR: docker not available — run: newgrp docker" >&2
    exit 1
  fi
  bash "$ROOT/scripts/run_full_import.sh"
  local estab
  estab=$(sg docker -c "docker exec receita-postgres psql -U receita_user -d receita_db -At -c 'SELECT count(*) FROM estabelecimentos'" | tr -d '[:space:]')
  log "estabelecimentos=$estab"
  if [[ -z "$estab" || "$estab" -lt "$MIN_ESTAB" ]]; then
    log "ERROR: estabelecimentos=$estab below $MIN_ESTAB — import incomplete" >&2
    exit 1
  fi
}

run_with_docker() {
  if docker info >/dev/null 2>&1; then
    "$@"
    return
  fi
  if groups | grep -qw docker; then
    sg docker -c "$(printf '%q ' "$@")"
    return
  fi
  log "ERROR: docker not available — run: newgrp docker" >&2
  exit 1
}

run_sync() {
  local vps_host="${VPS_HOST:-72.60.147.2}"
  local vps_user="${VPS_USER:-root}"
  log "Dump + upload to VPS (DUMP_TAG=$DUMP_TAG)"
  run_with_docker env FORCE_DUMP=1 DUMP_TAG="$DUMP_TAG" MIN_ESTABELECIMENTOS="$MIN_ESTAB" \
    bash "$ROOT/scripts/pc_to_vps_sync.sh" --upload-only
  log "Syncing restore script + running RESTORE_ONLY on VPS"
  scp -o BatchMode=yes "$ROOT/scripts/vps_first_deploy.sh" \
    "${vps_user}@${vps_host}:/opt/opencnpj/scripts/vps_first_deploy.sh"
  ssh -o BatchMode=yes "${vps_user}@${vps_host}" \
    "DUMP_TAG='$DUMP_TAG' RESTORE_ONLY=1 bash /opt/opencnpj/scripts/vps_first_deploy.sh"
  log "VPS restore finished — smoke AMAGGI + BB AC"
  ssh -o BatchMode=yes "${vps_user}@${vps_host}" 'bash -s' <<'EOF'
set -euo pipefail
KEY=$(grep -oE 'ocnpj_live_[a-f0-9]+' /etc/opencnpj/credentials.txt | head -1)
for cnpj in 77294254004343 00000000348945 10000000000145 60701190000104; do
  curl -sS -H "X-API-Key: $KEY" "http://127.0.0.1:8081/api/v1/cnpj/${cnpj}" \
    | python3 -c "import sys,json;d=json.load(sys.stdin);print(d.get('cnpj'), d.get('razao_social'), d.get('uf'), d.get('error'))"
done
docker exec opencnpj-postgres psql -U postgres -d opencnpj_cnpj -At -c "
SELECT 'empresas='||count(*) FROM empresas
UNION ALL SELECT 'estabelecimentos='||count(*) FROM estabelecimentos
UNION ALL SELECT 'socios='||count(*) FROM socios
UNION ALL SELECT 'simples='||count(*) FROM simples;"
EOF
}

case "$MODE" in
  full)
    wait_download
    assert_no_truncated_csvs
    run_import
    run_sync
    ;;
  import)
    assert_no_truncated_csvs
    run_import
    ;;
  sync)
    run_sync
    ;;
esac

log "Pipeline mode=$MODE done."
