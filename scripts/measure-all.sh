#!/usr/bin/env bash
# Runs the full measurement chain used in docs/BENCHMARKS.md, sequentially so runs do not
# interfere: warm query benchmarks per profile, a cold-cache run, HTTP load tests (oha)
# and the ZIP/CSV/Parquet/SQLite baselines.
#
# Usage: scripts/measure-all.sh BIN DATA_DIR COMPETENCE   (e.g. ... data 2026-10)
set -euo pipefail
BIN=${1:?usage: measure-all.sh BIN DATA_DIR COMPETENCE}
DATA=${2:?}
C=${3:?}
OUT="$DATA/bench"
mkdir -p "$OUT"

for p in balanced fast compact; do
  "$BIN" bench --snapshot "$DATA/snapshots/$C-$p.ocnpj" --queries 500 --threads 1,4,16 \
    --out "$OUT/$p-warm" --label "$p-warm" 2> "$OUT/$p-warm.log"
done
"$BIN" bench --snapshot "$DATA/snapshots/$C-balanced.ocnpj" --queries 200 --threads 1 --cold \
  --out "$OUT/balanced-cold" --label balanced-cold 2> "$OUT/balanced-cold.log"

# HTTP: same sampled workload replayed through the API.
port=18090
"$BIN" bench --snapshot "$DATA/snapshots/$C-balanced.ocnpj" --queries 2000 --write-urls "$OUT/urls" \
  --base-url "http://127.0.0.1:$port" 2> /dev/null
t0=$(date +%s.%N)
"$BIN" serve --snapshot "$DATA/snapshots/$C-balanced.ocnpj" --bind "127.0.0.1:$port" > "$OUT/serve.log" 2>&1 &
srv=$!
until curl -sf "http://127.0.0.1:$port/readyz" > /dev/null; do sleep 0.05; done
echo "startup_to_ready_s $(echo "$(date +%s.%N) - $t0" | bc)" > "$OUT/http-startup.txt"
for cat in lookup_cnpj name_prefix_typeahead filter_selective; do
  for c in 16 64; do
    oha --no-tui -j -z 20s -c "$c" --urls-from-file "$OUT/urls/$cat.txt" > "$OUT/http-$cat-c$c.json"
  done
done
curl -s "http://127.0.0.1:$port/metrics" > "$OUT/http-metrics.txt"
grep VmRSS /proc/$srv/status >> "$OUT/http-startup.txt" || true
kill "$srv"
wait "$srv" 2> /dev/null || true

"$(dirname "$0")/baselines.sh" "$DATA/raw/$C" "$DATA/baselines" 9 > "$DATA/baselines.log" 2>&1
echo "measurements done"
