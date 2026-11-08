#!/usr/bin/env python3
"""Run the MT official-data acceptance corpus against a local .ocind directory.

Usage:
    python3 scripts/official-data/run_acceptance.py \
        --dir /data/opencnpj-data/intelligence/official \
        --bin target/debug/opencnpj-intel
"""

import argparse
import json
import math
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CASES = ROOT / "docs/intelligence/results/official-data-mt/acceptance/cases.json"
OUT = ROOT / "docs/intelligence/results/official-data-mt/acceptance/results.json"


def run_case(binary, data_dir, case):
    if case["kind"] == "query":
        args = [binary, "official", "query", "--dir", data_dir, "--json", *case["args"]]
    else:
        args = [binary, "official", "ask", "--dir", data_dir, "--json", "--text", case["text"]]
    proc = subprocess.run(args, capture_output=True, text=True, timeout=120)
    record = {"id": case["id"], "kind": case["kind"], "expected_status": case["expected_status"], "exit": proc.returncode}
    if proc.returncode != 0:
        record.update({"status": None, "passed": False, "stderr": proc.stderr.strip()[:2000]})
        return record
    try:
        answer = json.loads(proc.stdout)
    except json.JSONDecodeError:
        record.update({"status": None, "passed": False, "stdout": proc.stdout.strip()[:2000], "stderr": proc.stderr.strip()[:2000]})
        return record
    record["status"] = answer.get("status")
    record["operation"] = answer.get("operation")
    record["unit"] = answer.get("unit")
    rows = answer.get("rows") or []
    record["rows"] = len(rows)
    if rows:
        record["first"] = {
            key: rows[0].get(key)
            for key in ("geography_code", "geography_name", "period", "value", "status", "category_label")
        }
    passed = answer.get("status") == case["expected_status"]
    expected_value = case.get("expected_first_value")
    if passed and expected_value is not None:
        value = rows[0].get("value") if rows else None
        passed = value is not None and math.isclose(float(value), float(expected_value), rel_tol=1e-9, abs_tol=1e-9)
        record["expected_first_value"] = expected_value
    expected_unit = case.get("expected_unit")
    if passed and expected_unit is not None and answer.get("unit") != expected_unit:
        passed = False
        record["expected_unit"] = expected_unit
    record["passed"] = passed
    if not passed:
        record["text"] = (answer.get("text") or "")[:1200]
    return record


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dir", default="/data/opencnpj-data/intelligence/official")
    parser.add_argument("--bin", default=os.environ.get("OPENCNPJ_INTEL_BIN", str(ROOT / "target/debug/opencnpj-intel")))
    parser.add_argument("--output", default=str(OUT))
    args = parser.parse_args()
    cases = json.loads(CASES.read_text())
    results = [run_case(args.bin, args.dir, case) for case in cases]
    passed = sum(1 for result in results if result["passed"])
    output = {
        "cases": len(results),
        "passed": passed,
        "failed": len(results) - passed,
        "data_dir": args.dir,
        "binary": args.bin,
        "results": results,
    }
    Path(args.output).write_text(json.dumps(output, ensure_ascii=False, indent=2) + "\n")
    print(f"{passed}/{len(results)} passed; wrote {args.output}")
    for result in results:
        if not result["passed"]:
            print(f"FAIL {result['id']}: expected {result['expected_status']}, got {result.get('status')}; {result.get('stderr') or result.get('text','')[:300]}")
    return 0 if passed == len(results) else 1


if __name__ == "__main__":
    sys.exit(main())
