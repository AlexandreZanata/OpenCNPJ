#!/usr/bin/env python3
"""Update per-approach benchmark docs from benchmark_comparison.tsv."""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TSV = ROOT / "data" / "benchmark_comparison.tsv"
DOCS = {
    "A01": ROOT / "docs/benchmarks/approach-01-optimized-parallel.md",
    "A02": ROOT / "docs/benchmarks/approach-02-sequential-files.md",
    "A03": ROOT / "docs/benchmarks/approach-03-large-batch.md",
    "A04": ROOT / "docs/benchmarks/approach-04-max-workers.md",
    "A05": ROOT / "docs/benchmarks/approach-05-no-pg-tuning.md",
}

rows = {}
for line in TSV.read_text().strip().splitlines()[1:]:
    aid, pct, wall, total, rps, run_at = line.split("\t")[:6]
    rows.setdefault(aid, {})[int(float(pct))] = {
        "wall": wall, "rows": total, "rps": rps, "run_at": run_at
    }

for aid, path in DOCS.items():
    if not path.exists():
        continue
    text = path.read_text()
    r10 = rows.get(aid, {}).get(10, {})
    r20 = rows.get(aid, {}).get(20, {})
    table = (
        "| Sample | Wall (s) | Rows | RPS | Run at |\n"
        "|--------|----------|------|-----|--------|\n"
        f"| 10% | {r10.get('wall', '_pending_')} | {r10.get('rows', '_pending_')} | {r10.get('rps', '_pending_')} | {r10.get('run_at', '_pending_')} |\n"
        f"| 20% | {r20.get('wall', '_pending_')} | {r20.get('rows', '_pending_')} | {r20.get('rps', '_pending_')} | {r20.get('run_at', '_pending_')} |"
    )
    text = re.sub(
        r"\| Sample \| Wall \(s\) \| Rows \| RPS \| Run at \|[\s\S]*?(?=\n## )",
        table + "\n",
        text,
        count=1,
    )
    path.write_text(text)
    print(f"updated {path.name}")
