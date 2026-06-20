#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

GUARD_ENABLED="${GUARD_ENABLED:-true}"
START_FROM="${START_FROM:-A01}"
SKIP_RESET="${SKIP_RESET:-false}"
CONF="${ROOT}/scripts/benchmark_approaches.conf"
OUT="${ROOT}/data/benchmark_comparison.tsv"
REPORT="${ROOT}/docs/benchmarks/COMPARISON.md"

if [[ "$GUARD_ENABLED" == "true" ]]; then
  # shellcheck source=scripts/lib/system_guard.sh
  source "$ROOT/scripts/lib/system_guard.sh"
  echo "==> System guard status (pre-flight)"
  guard_status
  if ! guard_preflight; then
    echo "[guard] suite aborted — free memory before running benchmarks"
    exit 137
  fi
  bash "$ROOT/scripts/system_guard.sh" daemon --pid $$ &
  SUITE_GUARD_PID=$!
  trap 'kill "$SUITE_GUARD_PID" 2>/dev/null || true' EXIT
fi

if [[ "$SKIP_RESET" != "true" ]]; then
  echo "==> Reset comparison file"
  echo -e "approach_id\tsample_pct\twall_sec\trows\trps\trun_at" > "$OUT"
else
  echo "==> Keeping existing results in $OUT (SKIP_RESET=true)"
  [[ -f "$OUT" ]] || echo -e "approach_id\tsample_pct\twall_sec\trows\trps\trun_at" > "$OUT"
fi

started=false
mapfile -t _approach_lines < <(grep -v '^#' "$CONF" | grep -v '^[[:space:]]*$')
for _line in "${_approach_lines[@]}"; do
  IFS='|' read -r id name workers batch tune drop <<< "$_line"
  [[ -z "$id" ]] && continue
  if [[ "$id" == "$START_FROM" ]]; then started=true; fi
  [[ "$started" == "true" ]] || continue
  for sample in 10 20; do
    if [[ "$GUARD_ENABLED" == "true" ]] && [[ -f "${GUARD_STATE:-$ROOT/data/system_guard.state}" ]] && \
       grep -q '^abort:' "${GUARD_STATE:-$ROOT/data/system_guard.state}" 2>/dev/null; then
      echo "[guard] stopping suite — previous abort detected"
      break 2
    fi

    if awk -F'\t' -v id="$id" -v s="$sample" '$1==id && $2==s {found=1} END{exit !found}' "$OUT" 2>/dev/null; then
      echo ">>> Skipping $id ($name) @ ${sample}% (already recorded)"
      continue
    fi

    echo ""
    echo ">>> Running $id ($name) @ ${sample}%"
    if [[ "$GUARD_ENABLED" == "true" ]]; then
      echo "==> Cooldown 30s (memory recovery)"
      sleep 30
      if ! guard_preflight; then
        echo "[guard] waiting 60s for memory..."
        sleep 60
        guard_preflight || { echo "[guard] still unsafe — stopping suite"; break 2; }
      fi
      echo "normal" > "${GUARD_STATE:-$ROOT/data/system_guard.state}"
    fi

    if ! APPROACH_ID="$id" SAMPLE_PERCENT="$sample" \
      IMPORT_WORKERS="$workers" IMPORT_BATCH_SIZE="$batch" \
      IMPORT_TUNE="$tune" DROP_INDEXES="$drop" \
      GUARD_ENABLED="$GUARD_ENABLED" \
      TARGET_SEC="$([[ "$sample" == "10" ]] && echo 180 || echo 360)" \
      bash scripts/benchmark_import_sample.sh; then
      rc=$?
      if [[ "$rc" -eq 137 ]]; then
        echo "[guard] suite stopped after guard abort (exit 137)"
        break 2
      fi
      exit "$rc"
    fi
  done
done

if [[ -n "${SUITE_GUARD_PID:-}" ]]; then
  kill "$SUITE_GUARD_PID" 2>/dev/null || true
  wait "$SUITE_GUARD_PID" 2>/dev/null || true
fi

python3 - "$OUT" "$REPORT" <<'PY'
import sys
from pathlib import Path
from datetime import datetime

tsv, report = Path(sys.argv[1]), Path(sys.argv[2])
names = {}
for line in Path(tsv.parent.parent / "scripts/benchmark_approaches.conf").read_text().splitlines():
    if not line or line.startswith("#"):
        continue
    pid, name, *_ = line.split("|")
    names[pid] = name

rows = []
lines_raw = tsv.read_text().strip().splitlines()
if len(lines_raw) <= 1:
    report.write_text("# Benchmark Comparison — incomplete (guard abort or no runs)\n")
    print(f"Report written (partial): {report}")
    sys.exit(0)

for line in lines_raw[1:]:
    aid, pct, wall, total, rps, *_ = line.split("\t")
    rows.append(dict(id=aid, pct=int(float(pct)), wall=float(wall), rows=int(total), rps=float(rps), name=names.get(aid, aid)))

by_sample = {10: sorted([r for r in rows if r["pct"] == 10], key=lambda r: r["wall"]),
             20: sorted([r for r in rows if r["pct"] == 20], key=lambda r: r["wall"])}

def medal(sample):
    if not by_sample[sample]:
        return "N/A", "N/A"
    best = by_sample[sample][0]
    return best["id"], best["name"]

b10, n10 = medal(10)
b20, n20 = medal(20)

lines = [
    "# Benchmark Comparison — 5 Import Approaches",
    "",
    f"Generated: {datetime.now().isoformat(timespec='seconds')}",
    "",
    "## Winner",
    "",
    f"- **10% fastest:** `{b10}` — {n10}",
    f"- **20% fastest:** `{b20}` — {n20}",
    "",
    "## Results at 10%",
    "",
    "| Rank | Approach | Wall (s) | Rows | RPS | vs best |",
    "|------|----------|----------|------|-----|---------|",
]
best10 = by_sample[10][0]["wall"] if by_sample[10] else 1
for i, r in enumerate(by_sample[10], 1):
    delta = ((r["wall"] / best10) - 1) * 100
    lines.append(f"| {i} | {r['id']} {r['name']} | {r['wall']} | {r['rows']:,} | {r['rps']:,.0f} | +{delta:.1f}% |")

lines += ["", "## Results at 20%", "",
          "| Rank | Approach | Wall (s) | Rows | RPS | vs best |",
          "|------|----------|----------|------|-----|---------|"]
best20 = by_sample[20][0]["wall"] if by_sample[20] else 1
for i, r in enumerate(by_sample[20], 1):
    delta = ((r["wall"] / best20) - 1) * 100
    lines.append(f"| {i} | {r['id']} {r['name']} | {r['wall']} | {r['rows']:,} | {r['rps']:,.0f} | +{delta:.1f}% |")

lines += ["", "## Scaling 10% → 20%", "",
          "| Approach | 10% (s) | 20% (s) | Time +% | Rows +% | Linearity |"]
for aid in sorted({r["id"] for r in rows}):
    r10 = next((r for r in rows if r["id"] == aid and r["pct"] == 10), None)
    r20 = next((r for r in rows if r["id"] == aid and r["pct"] == 20), None)
    if not r10 or not r20:
        continue
    t_pct = (r20["wall"] / r10["wall"] - 1) * 100
    row_pct = (r20["rows"] / r10["rows"] - 1) * 100
    lin = (r20["wall"] / r10["wall"]) / (r20["rows"] / r10["rows"]) * 100 if r10["rows"] else 0
    lines.append(f"| {aid} {names.get(aid,'')} | {r10['wall']} | {r20['wall']} | +{t_pct:.1f}% | +{row_pct:.1f}% | {lin:.1f}% |")

lines += ["", "## Raw data", "", f"See `{tsv.relative_to(tsv.parent.parent)}`"]
report.write_text("\n".join(lines) + "\n")
print(f"Report written: {report}")
PY

echo "Done. See $REPORT"
