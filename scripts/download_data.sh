#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUTPUT="${DATA_PATH:-./data}"
MONTH="${DOWNLOAD_MONTH:-}"
EXTRA_ARGS=("$@")

mkdir -p "$OUTPUT"

CMD=(go run ./cmd/downloader --output="$OUTPUT")
if [[ -n "$MONTH" ]]; then
  CMD+=(--month="$MONTH")
fi
CMD+=("${EXTRA_ARGS[@]}")

echo "==> Baixando dados públicos CNPJ da Receita Federal"
echo "    destino: $OUTPUT"
exec "${CMD[@]}"
