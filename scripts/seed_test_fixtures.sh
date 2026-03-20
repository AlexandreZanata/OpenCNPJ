#!/usr/bin/env bash
set -euo pipefail

DATA_DIR="${1:-./data}"
OUT_DIR="${2:-./tests/fixtures}"
LINES="${3:-10000}"

mkdir -p "${OUT_DIR}"

for pattern in EMPRECSV ESTABELE SOCIOCSV SIMPLES MEI; do
  file="$(ls "${DATA_DIR}"/*"${pattern}" 2>/dev/null | head -n 1 || true)"
  if [[ -n "${file}" ]]; then
    out="${OUT_DIR}/$(basename "${file}").sample.csv"
    head -n "${LINES}" "${file}" > "${out}"
    echo "generated ${out}"
  fi
done
