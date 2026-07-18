#!/usr/bin/env bash
# E2E lookup with per-query latency stats (p50/p95/max).
# Usage:
#   OPENCNPJ_E2E_BASE_URL=http://127.0.0.1:8081 \
#   OPENCNPJ_E2E_API_KEY=... \
#   ./scripts/e2e_cnpj_lookup_latency.sh
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="${FIXTURE:-$ROOT/testdata/e2e/cnpj_lookup_50.json}"
BASE_URL="${OPENCNPJ_E2E_BASE_URL:?set OPENCNPJ_E2E_BASE_URL}"
API_KEY="${OPENCNPJ_E2E_API_KEY:?set OPENCNPJ_E2E_API_KEY}"
OUT_TSV="${OUT_TSV:-/tmp/opencnpj_lookup_latency.tsv}"
BASE_URL="${BASE_URL%/}"

python3 - "$FIXTURE" "$BASE_URL" "$API_KEY" "$OUT_TSV" <<'PY'
import json, statistics, sys, time, urllib.error, urllib.request

fixture, base, key, out_tsv = sys.argv[1:5]
cases = json.load(open(fixture, encoding="utf-8"))["cases"]
rows = []
ok = fail = 0
print("cnpj\thttp\tms\texpect\tuf\tlabel")
for case in cases:
    cnpj = case["cnpj"]
    expect = int(case["expect_http"])
    url = f"{base}/api/v1/cnpj/{cnpj}"
    req = urllib.request.Request(
        url,
        headers={
            "X-API-Key": key,
            "Accept": "application/json",
            "User-Agent": "OpenCNPJ-Latency/1.0",
        },
    )
    code = 0
    for attempt in range(5):
        t0 = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=20) as resp:
                code = resp.getcode()
                resp.read()
        except urllib.error.HTTPError as e:
            code = e.code
            e.read()
        ms = (time.perf_counter() - t0) * 1000
        if code != 429:
            break
        time.sleep(1.2 * (attempt + 1))
    label = (case.get("label") or "")[:40]
    uf = case.get("uf") or ""
    status = "OK" if code == expect else "FAIL"
    if status == "OK":
        ok += 1
    else:
        fail += 1
    print(f"{cnpj}\t{code}\t{ms:.1f}\t{expect}\t{uf}\t{label}")
    rows.append({"cnpj": cnpj, "http": code, "ms": ms, "expect": expect, "ok": status == "OK"})
    time.sleep(0.35)

with open(out_tsv, "w", encoding="utf-8") as f:
    f.write("cnpj\thttp\tms\texpect\tok\n")
    for r in rows:
        f.write(f"{r['cnpj']}\t{r['http']}\t{r['ms']:.3f}\t{r['expect']}\t{int(r['ok'])}\n")

lat = sorted(r["ms"] for r in rows)
hit = sorted(r["ms"] for r in rows if r["http"] == 200)
miss = sorted(r["ms"] for r in rows if r["http"] in (404, 400))

def pct(vals, p):
    if not vals:
        return float("nan")
    k = (len(vals) - 1) * p / 100
    f = int(k)
    c = min(f + 1, len(vals) - 1)
    if f == c:
        return vals[f]
    return vals[f] + (vals[c] - vals[f]) * (k - f)

print("---")
print(f"passed={ok} failed={fail} total={len(rows)} tsv={out_tsv}")
print(f"all_ms p50={pct(lat,50):.1f} p95={pct(lat,95):.1f} max={lat[-1]:.1f} mean={statistics.fmean(lat):.1f}")
if hit:
    print(f"http200_ms p50={pct(hit,50):.1f} p95={pct(hit,95):.1f} max={hit[-1]:.1f} n={len(hit)}")
if miss:
    print(f"http4xx_ms p50={pct(miss,50):.1f} p95={pct(miss,95):.1f} max={miss[-1]:.1f} n={len(miss)}")
if fail:
    raise SystemExit(1)
print("latency E2E passed")
PY
