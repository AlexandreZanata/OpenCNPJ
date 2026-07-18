#!/usr/bin/env bash
# E2E: validate GET /api/v1/cnpj/{cnpj} against fixture (≥1 UF coverage).
# Usage:
#   OPENCNPJ_E2E_BASE_URL=https://api.comerc.app.br \
#   OPENCNPJ_E2E_API_KEY=ocnpj_live_... \
#   ./scripts/e2e_cnpj_lookup_validation.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FIXTURE="${FIXTURE:-$ROOT/testdata/e2e/cnpj_lookup_50.json}"
BASE_URL="${OPENCNPJ_E2E_BASE_URL:?set OPENCNPJ_E2E_BASE_URL}"
API_KEY="${OPENCNPJ_E2E_API_KEY:?set OPENCNPJ_E2E_API_KEY}"
BASE_URL="${BASE_URL%/}"

python3 - "$FIXTURE" "$BASE_URL" "$API_KEY" <<'PY'
import json, os, sys, time, urllib.request, urllib.error

fixture_path, base, key = sys.argv[1], sys.argv[2], sys.argv[3]
with open(fixture_path, encoding="utf-8") as f:
    data = json.load(f)
cases = data["cases"]
assert len(cases) == 50, f"want 50 cases, got {len(cases)}"
ufs = {c.get("uf") for c in cases if c.get("uf")}
required = {
    "AC","AL","AP","AM","BA","CE","DF","ES","GO","MA","MT","MS","MG",
    "PA","PB","PR","PE","PI","RJ","RN","RS","RO","RR","SC","SP","SE","TO",
}
missing_ufs = sorted(required - ufs)
if missing_ufs:
    raise SystemExit(f"fixture missing UFs: {missing_ufs}")

ok = fail = 0
failures = []
for i, case in enumerate(cases, 1):
    cnpj = case["cnpj"]
    expect = int(case["expect_http"])
    url = f"{base}/api/v1/cnpj/{cnpj}"
    req = urllib.request.Request(
        url,
        headers={
            "X-API-Key": key,
            "Accept": "application/json",
            "User-Agent": "OpenCNPJ-E2E/1.0 (+https://github.com/AlexandreZanata/OpenCNPJ)",
        },
    )
    code, body, elapsed = None, "", 0.0
    for attempt in range(4):
        t0 = time.perf_counter()
        try:
            with urllib.request.urlopen(req, timeout=15) as resp:
                code = resp.getcode()
                body = resp.read().decode("utf-8", errors="replace")
        except urllib.error.HTTPError as e:
            code = e.code
            body = e.read().decode("utf-8", errors="replace")
        except Exception as e:
            fail += 1
            failures.append(f"{cnpj}: request error {e}")
            print(f"[{i:02d}/50] FAIL {cnpj} error={e}")
            code = None
            break
        elapsed = (time.perf_counter() - t0) * 1000
        if code != 429:
            break
        time.sleep(1.5 * (attempt + 1))
    if code is None:
        continue
    label = case.get("label", "")[:40]
    if code != expect:
        fail += 1
        failures.append(f"{cnpj}: http {code} want {expect} ({label}) body={body[:120]}")
        print(f"[{i:02d}/50] FAIL {cnpj} http={code} want={expect} {elapsed:.0f}ms {label}")
    else:
        ok += 1
        print(f"[{i:02d}/50] OK   {cnpj} http={code} {elapsed:.0f}ms {label}")
    time.sleep(0.35)

print("---")
print(f"passed={ok} failed={fail} total={len(cases)} ufs={len(ufs)}")
if failures:
    print("FAILURES:")
    for line in failures:
        print(" -", line)
    raise SystemExit(1)
print("E2E lookup validation passed")
PY
