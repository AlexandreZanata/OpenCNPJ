#!/bin/sh
# Run the evaluation tasks of docs/intelligence/EVALUATION-TASKS.md and keep the evidence.
#
# Usage: scripts/eval/run_tasks.sh BASELINE_BIN CANDIDATE_BIN OUT_DIR [DATA_DIR]
#   BASELINE_BIN   opencnpj-intel built from main (ET-02 compares against it)
#   CANDIDATE_BIN  opencnpj-intel built from the change under review
#   OUT_DIR        evidence directory (bulky results.jsonl files are removed at the end)
#   DATA_DIR       optional, e.g. /data/opencnpj-data: enables the real-data tasks (ET-04)
# Run from the repository root. The baseline reads main's sector catalogue, extracted with git.
set -eu
BASE=$1 CAND=$2 OUT=$3 DATA=${4:-}
SETS=$(ls crates/opencnpj-lab/eval/commercial-round-*/questions.jsonl crates/opencnpj-lab/eval/adv.jsonl \
  crates/opencnpj-lab/eval/dialogues.jsonl crates/opencnpj-lab/eval/h-informal.jsonl crates/opencnpj-lab/eval/ood.jsonl \
  crates/opencnpj-lab/eval/skills-*/questions.jsonl crates/opencnpj-lab/eval/probes-*/questions.jsonl)
mkdir -p "$OUT/baseline" "$OUT/candidate"
git show origin/main:crates/opencnpj-insights/catalog/sectors.toml > "$OUT/baseline/sectors.toml"
name() { echo "$1" | sed 's#.*/eval/##; s#/questions.jsonl##; s#.jsonl##'; }

echo "ET-02/ET-06 interpretation regression and skill scores"
for set in $SETS; do
  n=$(name "$set")
  "$BASE" eval-questions --interpret-only --sectors "$OUT/baseline/sectors.toml" --set "$set" --out "$OUT/baseline/$n" > /dev/null 2>&1 || true
  "$CAND" eval-questions --interpret-only --set "$set" --out "$OUT/candidate/$n" > /dev/null
done
python3 scripts/eval/regression_diff.py "$OUT/baseline" "$OUT/candidate" "$OUT" || echo "ET-02 FAILED: items got worse (see $OUT/changes.jsonl)"

echo "ET-03 interpretation contracts on every set"
CONTRACT_SETS=$(for set in $SETS; do printf ' --set %s' "$set"; done)
# shellcheck disable=SC2086
"$CAND" contracts $CONTRACT_SETS --out "$OUT/contracts" > /dev/null

if [ -n "$DATA" ]; then
  echo "ET-04 data contracts on the snapshot (every 10th new question)"
  # shellcheck disable=SC2086
  "$CAND" contracts --agg "$DATA/intelligence/agg/2026-10.ocagg" --history "$DATA/intelligence/backfill/aggregates" \
    --snapshot "$DATA/snapshots/2026-10-balanced-1.2.ocnpj" --data-every 10 \
    --set crates/opencnpj-lab/eval/commercial-round-01/questions.jsonl --set crates/opencnpj-lab/eval/commercial-round-02/questions.jsonl \
    --set crates/opencnpj-lab/eval/h-informal.jsonl --set crates/opencnpj-lab/eval/skills-export-01/questions.jsonl \
    --set crates/opencnpj-lab/eval/skills-company-01/questions.jsonl --set crates/opencnpj-lab/eval/probes-01/questions.jsonl \
    --out "$OUT/data-contracts" > /dev/null
fi

# Keep summaries and changes only: per-item outputs of national sets are large.
find "$OUT" \( -name results.jsonl -o -name predictions.jsonl \) -delete
echo "evidence in $OUT"
