#!/usr/bin/env python3
"""Renders Markdown tables from measurement outputs (bench.json, build reports, oha JSON,
baselines.tsv). Usage: python3 -I scripts/report.py DATA_DIR COMPETENCE > tables.md"""
import csv
import glob
import json
import os
import sys

data, comp = sys.argv[1], sys.argv[2]
P = ["fast", "balanced", "compact"]


def gb(b):
    return f"{b / 1e9:.2f}"


def load(path):
    with open(path) as f:
        return json.load(f)


print("### Build (national)\n")
print("| profile | file GB | payload GB | postings GB | terms MB | other MB | encode s | index merge s | wall s | peak RSS GB | peak temp GB |")
print("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
for p in P:
    r = load(f"{data}/reports/build-{comp}-{p}.json")
    s, t = r["section_bytes"], r["timings_s"]
    other = r["file_bytes"] - s["data"] - s["postings"] - s["terms"]
    print(f"| {p} | {gb(r['file_bytes'])} | {gb(s['data'])} | {gb(s['postings'])} | {s['terms'] / 1e6:.1f} | {other / 1e6:.1f} | "
          f"{t['encode_blocks']:.0f} | {t['merge_index']:.0f} | {t['total_wall']:.0f} | {r['peak_rss_bytes'] / 1e9:.2f} | {r['peak_temp_bytes'] / 1e9:.1f} |")

print("\n### Queries, warm cache (national, in-process: search + fetch + JSON)\n")
cats = None
rows = {}
for p in P:
    j = load(f"{data}/bench/{p}-warm/bench.json")
    for r in j["results"]:
        rows[(p, r["category"], r["threads"], r["cache"])] = r
    cats = sorted({r["category"] for r in j["results"]})
print("| category | " + " | ".join(f"{p} p50 / p95 / p99 ms" for p in P) + " | balanced q/s @1/4/16 threads |")
print("|---|" + "---:|" * (len(P) + 1))
for c in cats:
    cells = []
    for p in P:
        r = rows.get((p, c, 1, "warm"))
        cells.append(f"{r['p50_ms']:.3f} / {r['p95_ms']:.3f} / {r['p99_ms']:.3f}" if r else "–")
    q = [rows.get(("balanced", c, t, "warm")) for t in (1, 4, 16)]
    cells.append(" / ".join(f"{x['qps']:.0f}" if x else "–" for x in q))
    print(f"| {c} | " + " | ".join(cells) + " |")

cold = f"{data}/bench/balanced-cold/bench.json"
if os.path.exists(cold):
    j = load(cold)
    print(f"\n### Cold cache (balanced, page cache evicted before each category; open {j['open_ms']:.1f} ms)\n")
    print("| category | p50 ms | p95 ms | p99 ms |\n|---|---:|---:|---:|")
    for r in j["results"]:
        if r["cache"] == "cold":
            print(f"| {r['category']} | {r['p50_ms']:.3f} | {r['p95_ms']:.3f} | {r['p99_ms']:.3f} |")

http = sorted(glob.glob(f"{data}/bench/http-*.json"))
if http:
    print("\n### HTTP (oha, 20 s, balanced, loopback)\n")
    print("| workload | connections | req/s | p50 ms | p95 ms | p99 ms | success |\n|---|---:|---:|---:|---:|---:|---:|")
    for f in http:
        j = load(f)
        name = os.path.basename(f)[5:-5]
        cat, c = name.rsplit("-c", 1)
        s, l = j["summary"], j["latencyPercentiles"]
        print(f"| {cat} | {c} | {s['requestsPerSec']:.0f} | {l['p50'] * 1e3:.2f} | {l['p95'] * 1e3:.2f} | {l['p99'] * 1e3:.2f} | {s['successRate'] * 100:.1f}% |")

tsv = f"{data}/baselines/baselines.tsv"
if os.path.exists(tsv):
    print("\n### Baselines (raw)\n")
    print("| metric | value | note |\n|---|---:|---|")
    for row in csv.reader(open(tsv), delimiter="\t"):
        print(f"| {row[0]} | {row[1]} | {row[2] if len(row) > 2 else ''} |")
