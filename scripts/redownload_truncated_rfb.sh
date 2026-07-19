#!/usr/bin/env bash
# Force re-download of RFB archives whose CSVs were truncated at 512 MiB
# (historical maxZipMemberBytes bug). Clears .done markers + truncated CSVs,
# then runs the downloader for the given month.
#
# Usage:
#   ./scripts/redownload_truncated_rfb.sh
#   DOWNLOAD_MONTH=2026-06 ./scripts/redownload_truncated_rfb.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

DATA="${DATA_PATH:-$ROOT/data}"
MONTH="${DOWNLOAD_MONTH:-2026-06}"
LIMIT=$((512 * 1024 * 1024))

export PATH="${HOME}/.local/go/bin:${HOME}/go/bin:/usr/local/go/bin:${PATH}"

log() { echo "[$(date '+%H:%M:%S')] $*"; }

truncated_csvs=()
while IFS= read -r -d '' f; do
  truncated_csvs+=("$f")
done < <(find "$DATA" -maxdepth 1 -type f \( \
  -name '*.ESTABELE' -o -name '*SOCIOCSV' -o -name '*SIMPLES*' \
  \) -size "${LIMIT}c" -print0 2>/dev/null)

if [[ ${#truncated_csvs[@]} -eq 0 ]]; then
  log "No 512 MiB truncated ESTABELE/SOCIO/SIMPLES CSVs under $DATA"
else
  log "Removing ${#truncated_csvs[@]} truncated CSV(s)"
  for f in "${truncated_csvs[@]}"; do
    log "  rm $(basename "$f")"
    rm -f "$f"
  done
fi

# Archives that historically hit the 512 MiB extract cap.
archives=(
  Estabelecimentos0.zip Estabelecimentos1.zip Estabelecimentos2.zip
  Estabelecimentos3.zip Estabelecimentos4.zip Estabelecimentos5.zip
  Estabelecimentos6.zip Estabelecimentos7.zip Estabelecimentos8.zip
  Estabelecimentos9.zip
  Socios0.zip
  Simples.zip
)

marker_dir="$DATA/.downloaded/$MONTH"
mkdir -p "$marker_dir"
for a in "${archives[@]}"; do
  marker="$marker_dir/${a}.done"
  if [[ -f "$marker" ]]; then
    log "Clear marker $MONTH/$a"
    rm -f "$marker"
  fi
  rm -f "$DATA/$a" "$DATA/${a}.part"
done

# Drop leftover Empresas0.zip after successful EMPRECSV repair (frees ~512 MiB).
rm -f "$DATA/Empresas0.zip" "$DATA/Empresas0.zip.part"

log "Re-downloading truncated archives for month=$MONTH → $DATA"
exec go run ./cmd/downloader --output="$DATA" --month="$MONTH"
