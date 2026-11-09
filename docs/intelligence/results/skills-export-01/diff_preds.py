import json, sys, os, collections
# Usage: diff_preds.py DIR [NEW]; DIR/baseline/<set>/ and DIR/<NEW>/<set>/ hold eval-questions outputs.
sp = sys.argv[1]; new = sys.argv[2] if len(sys.argv) > 2 else "new"
total_changed = 0
for name in sorted(os.listdir(f"{sp}/baseline")):
    b = f"{sp}/baseline/{name}/predictions.jsonl"
    n = f"{sp}/{new}/{name}/predictions.jsonl"
    if not os.path.exists(b) or not os.path.exists(n): continue
    load = lambda p: {r["id"]: r for r in map(json.loads, open(p))}
    B, N = load(b), load(n)
    changed = [i for i in B if (B[i]["decision"], B[i]["ir"]) != (N[i]["decision"], N[i]["ir"])]
    sb = json.load(open(f"{sp}/baseline/{name}/summary.json"))["counts"]
    sn = json.load(open(f"{sp}/{new}/{name}/summary.json"))["counts"]
    total_changed += len(changed)
    print(f"{name:22s} items={sb['items']:6d} correct {sb.get('interpretation_correct',0):6d} -> {sn.get('interpretation_correct',0):6d}  changed={len(changed)}")
    for i in changed[:4]:
        print("   ", i, B[i]["decision"], "->", N[i]["decision"], "| op", (B[i]["ir"] or {}).get("op"), "->", (N[i]["ir"] or {}).get("op"))
print("total changed", total_changed)
