#!/usr/bin/env bash
# Download the latest published CNPJ open data from Receita Federal (single command + progress bar).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

OUTPUT="${DATA_PATH:-./data}"
mkdir -p "$OUTPUT"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║  CNPJ Download — Receita Federal (latest available month)   ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""
echo "  Destination : $OUTPUT"
echo "  Progress    : live percentage on the line below"
echo "  Tip         : re-run safely — already downloaded ZIPs are skipped"
echo ""

exec go run ./cmd/downloader --output="$OUTPUT" "$@"
