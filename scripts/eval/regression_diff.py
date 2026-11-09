#!/usr/bin/env python3
"""Compare two `eval-questions --interpret-only` runs item by item.

Usage: regression_diff.py BASELINE_DIR CANDIDATE_DIR OUT_DIR

Each directory holds one sub-directory per evaluation set (results.jsonl, summary.json).
An item changed when its decision, request, company search or export differs. A change is
"better" when the item became correct, "worse" when it stopped being correct and
"neutral" otherwise. Writes regression.txt (one line per set and a TOTAL line) and
changes.jsonl (every changed item). Exit status 1 when any item got worse.
"""

import collections
import json
import os
import sys


def key(prediction):
    return tuple(json.dumps(prediction.get(field), sort_keys=True) for field in ("decision", "request", "company", "export"))


def main():
    baseline, candidate, out = sys.argv[1:4]
    os.makedirs(out, exist_ok=True)
    kinds = collections.Counter()
    lines = []
    with open(os.path.join(out, "changes.jsonl"), "w", encoding="utf-8") as changes:
        for name in sorted(os.listdir(candidate)):
            before_path = os.path.join(baseline, name, "results.jsonl")
            after_path = os.path.join(candidate, name, "results.jsonl")
            if not (os.path.exists(before_path) and os.path.exists(after_path)):
                continue
            before = {}
            for line in open(before_path, encoding="utf-8"):
                row = json.loads(line)
                before[row["id"]] = (row["interpretation_correct"], key(row["prediction"]))
            changed = 0
            for line in open(after_path, encoding="utf-8"):
                row = json.loads(line)
                if row["id"] not in before or before[row["id"]][1] == key(row["prediction"]):
                    continue
                changed += 1
                was, now = before[row["id"]][0], row["interpretation_correct"]
                kind = "better" if now and not was else "worse" if was and not now else "neutral"
                kinds[kind] += 1
                changes.write(json.dumps({"set": name, "id": row["id"], "kind": kind, "text": row["text"]}, ensure_ascii=False) + "\n")
            b = json.load(open(os.path.join(baseline, name, "summary.json")))["counts"]
            c = json.load(open(os.path.join(candidate, name, "summary.json")))["counts"]
            lines.append(
                f"{name}\titems={c['items']}\tbefore_correct={b.get('interpretation_correct', 0)}"
                f"\tafter_correct={c.get('interpretation_correct', 0)}\tchanged={changed}"
            )
    lines.append(f"TOTAL\tchanged={sum(kinds.values())}\t{dict(kinds)}")
    with open(os.path.join(out, "regression.txt"), "w", encoding="utf-8") as stream:
        stream.write("\n".join(lines) + "\n")
    print("\n".join(lines))
    sys.exit(1 if kinds["worse"] else 0)


if __name__ == "__main__":
    main()
