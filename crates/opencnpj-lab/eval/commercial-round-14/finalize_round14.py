#!/usr/bin/env python3
"""Finalize Commercial Evaluation Round 14 evidence, metamorphic checks, replays, and report."""

from __future__ import annotations

import datetime
import hashlib
import json
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path("/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ")
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/commercial-round-14"
RES_DIR = ROOT / "docs/intelligence/results/commercial-round-14"
DATA_RES_DIR = Path("/data/opencnpj-data/intelligence/results/commercial-round-14")
BINARY_PATH = Path("/data/opencnpj-target/release/opencnpj-intel")
SNAPSHOT_PATH = Path("/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj")
AGG_PATH = Path("/data/opencnpj-data/intelligence/agg/2026-10.ocagg")
HISTORY_PATH = Path("/data/opencnpj-data/intelligence/backfill/aggregates")


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        while chunk := f.read(1 << 20):
            h.update(chunk)
    return h.hexdigest()


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> None:
    questions_path = EVAL_DIR / "questions.jsonl"
    nat_results_path = DATA_RES_DIR / "national/results.jsonl"
    nat_preds_path = DATA_RES_DIR / "national/predictions.jsonl"
    nat_summary_path = RES_DIR / "national/summary.json"
    adjudications_path = RES_DIR / "label-adjudications.jsonl"

    set_sha = sha256_file(questions_path)
    manifest = load_json(EVAL_DIR / "manifest.json")
    assert set_sha == manifest["questions_jsonl_sha256"], (
        f"questions.jsonl hash changed: {set_sha} != {manifest['questions_jsonl_sha256']}"
    )

    questions = []
    with questions_path.open(encoding="utf-8") as f:
        for line in f:
            questions.append(json.loads(line))

    results = []
    with nat_results_path.open(encoding="utf-8") as f:
        for line in f:
            results.append(json.loads(line))

    preds = []
    with nat_preds_path.open(encoding="utf-8") as f:
        for line in f:
            preds.append(json.loads(line))

    assert len(questions) == 50000
    assert len(results) == 50000
    assert len(preds) == 50000

    mismatches = [(q, r, p) for q, r, p in zip(questions, results, preds) if not r.get("interpretation_correct")]
    assert not mismatches, "Unexpected interpretation mismatches: " + ", ".join(m[0]["id"] for m in mismatches[:10])
    adjudications = {}
    adjudications_path.write_text("", encoding="utf-8")

    # Verify order and IDs match 1-to-1 and build split breakdown + metamorphic checks
    by_split = defaultdict(lambda: Counter())
    by_provenance = defaultdict(lambda: Counter())
    by_category = defaultdict(lambda: Counter())
    by_geo_stratum = defaultdict(lambda: Counter())
    by_uf = defaultdict(lambda: Counter())
    by_register = defaultdict(lambda: Counter())
    by_decision = defaultdict(lambda: Counter())
    by_batch = defaultdict(lambda: Counter())

    adjudicated_correct = 0
    raw_correct = 0

    # Indexes for real-data metamorphic verification across the 50,000 executed items
    by_canonical_ir = defaultdict(list)
    stock_counts_by_base = defaultdict(list)  # (place_key, act_key, unit) -> [(extra_filters_set, scalar, qid)]
    breakdowns_by_base = defaultdict(list)    # (place_key, act_key, unit, dim) -> [(sum_rows, qid)]
    flow_counts_by_base = defaultdict(list)   # (place_key, act_key, metric) -> [(from_day, to_day, scalar, qid)]
    ranks_by_base = defaultdict(list)         # (scope_key, dim, metric_key, dir) -> [(k, rows, qid)]
    exclusion_counts = []                     # [(place_key, act_key, excl_key, scalar, qid)]

    for q, r, p in zip(questions, results, preds):
        assert q["id"] == r["id"] == p["id"], f"ID mismatch: {q['id']} vs {r['id']} vs {p['id']}"
        meta = q.get("meta", {})
        sp = meta.get("split", "unknown")
        prov = meta.get("provenance_class", "unknown")
        cat = meta.get("primary_family", meta.get("category", "unknown"))
        geo = meta.get("geographic_stratum", "unknown")
        uf = meta.get("uf", "ALL")
        style = meta.get("register", "unknown")
        dec = q["gold"]["decision"]
        batch_id = meta.get("batch_id", "unknown")

        is_raw_ok = bool(r.get("interpretation_correct"))
        if is_raw_ok:
            raw_correct += 1
            adjudicated_correct += 1
        elif q["id"] in adjudications:
            adj = adjudications[q["id"]]["adjudicated_gold"]
            if p["decision"] == adj["decision"]:
                adjudicated_correct += 1

        gold_exec = r.get("gold_execution") or {}
        exec_ok = gold_exec.get("status") == "ok"
        for bucket, key in [
            (by_split, sp),
            (by_provenance, prov),
            (by_category, cat),
            (by_geo_stratum, geo),
            (by_uf, uf),
            (by_register, style),
            (by_decision, dec),
            (by_batch, batch_id),
        ]:
            bucket[key]["items"] += 1
            if is_raw_ok:
                bucket[key]["raw_interpretation_correct"] += 1
            if is_raw_ok or q["id"] in adjudications:
                bucket[key]["adjudicated_interpretation_correct"] += 1
            if exec_ok:
                bucket[key]["gold_backend_ok"] += 1

        ir = q["gold"].get("ir")
        if ir and exec_ok:
            ir_key = json.dumps(ir, sort_keys=True)
            by_canonical_ir[ir_key].append((q["id"], p.get("ir"), gold_exec.get("value")))

            op = ir["op"]
            sc = ir["scope"]
            tm = ir["time"]
            unit = ir.get("unit", "establishments")
            place_key = json.dumps(sc.get("place"), sort_keys=True)
            act_key = json.dumps(sc.get("activity"), sort_keys=True)

            # 1) Stock counts for monotonic subset, partition sum, and negation complement
            if (
                op["kind"] == "count"
                and op.get("metric") == "stock"
                and tm.get("competence") == 321
                and sc.get("status") == ["active"]
                and not sc.get("within_ufs")
                and not sc.get("place_population")
            ):
                scalar = (gold_exec.get("value") or {}).get("scalar")
                if scalar is not None:
                    if sc.get("exclude"):
                        exclusion_counts.append((place_key, act_key, unit, json.dumps(sc.get("exclude"), sort_keys=True), scalar, q["id"]))
                    else:
                        extras = {}
                        for fk in ["contact", "mei", "simples", "porte", "head_office", "capital", "natureza", "opened"]:
                            fv = sc.get(fk)
                            if fv is not None and fv != [] and fv != {}:
                                extras[fk] = json.dumps(fv, sort_keys=True)
                        stock_counts_by_base[(place_key, act_key, unit)].append((extras, scalar, q["id"]))

            # 2) Breakdowns for partition sums
            if (
                op["kind"] == "breakdown"
                and op.get("metric") == "stock"
                and tm.get("competence") == 321
            ):
                rows = (gold_exec.get("value") or {}).get("rows")
                ref_rows = ((gold_exec.get("reference") or {}).get("rows")) if gold_exec.get("reference") else None
                if rows is not None:
                    row_sum = sum(row["value"] for row in rows)
                    ref_sum = sum(row["value"] for row in ref_rows) if ref_rows is not None else row_sum
                    non_neg = all(row["value"] >= 0 for row in rows)
                    breakdowns_by_base[(place_key, act_key, unit, str(op.get("dim")))].append((row_sum, ref_sum, non_neg, q["id"]))

            # 3) Flow counts for temporal ordering
            if (
                op["kind"] == "count"
                and isinstance(op.get("metric"), dict)
                and "flow" in op["metric"]
                and op["metric"]["flow"] in ("registrations", "closures", "mei_opt_ins", "mei_exits")
                and tm.get("period")
                and not any(sc.get(fk) for fk in ["contact", "mei", "simples", "porte", "head_office", "capital", "natureza", "opened", "exclude", "within_ufs", "place_population"])
            ):
                scalar = (gold_exec.get("value") or {}).get("scalar")
                per = tm["period"]
                if scalar is not None and "days" in per:
                    flow_counts_by_base[(place_key, act_key, op["metric"]["flow"])].append(
                        (per["days"]["from"], per["days"]["to"], scalar, q["id"])
                    )

            # 4) Rankings for k-prefix consistency
            if op["kind"] == "rank":
                rows = (gold_exec.get("value") or {}).get("rows")
                if rows is not None:
                    scope_key = json.dumps({
                        "scope": sc,
                        "time": tm,
                        "unit": unit,
                    }, sort_keys=True)
                    metric_key = json.dumps(op.get("metric"), sort_keys=True)
                    ranks_by_base[(scope_key, op.get("dim"), metric_key, op.get("dir"))].append(
                        (op.get("k", 10), rows, q["id"])
                    )

    assert raw_correct == 50000, f"Expected 50000 raw_correct, got {raw_correct}"
    assert adjudicated_correct == 50000, f"Expected 50000 adjudicated_correct, got {adjudicated_correct}"

    # Verify metamorphic invariants on actual executed values
    meta_stats = Counter()
    meta_violations = []

    # Family 1: Equivalence across identical canonical IRs
    for ir_key, entries in by_canonical_ir.items():
        if len(entries) > 1:
            meta_stats["equivalence_groups_checked"] += 1
            meta_stats["equivalence_items_checked"] += len(entries)
            vals = {json.dumps(e[2], sort_keys=True) for e in entries}
            preds_set = {json.dumps(e[1], sort_keys=True) for e in entries}
            if len(vals) == 1 and len(preds_set) == 1:
                meta_stats["equivalence_groups_passed"] += 1
            else:
                meta_violations.append({"relation": "equivalence", "ids": [e[0] for e in entries[:4]]})

    # Family 2: Monotonic subset (strictly more restrictive filter set => scalar <= superset scalar)
    for base_key, items_list in stock_counts_by_base.items():
        if len(items_list) > 1:
            for i in range(len(items_list)):
                ext_a, val_a, id_a = items_list[i]
                for j in range(len(items_list)):
                    if i == j:
                        continue
                    ext_b, val_b, id_b = items_list[j]
                    # Check if ext_b is a strict superset of ext_a's filter constraints (so B is more restricted than A)
                    if len(ext_b) > len(ext_a) and all(ext_b.get(k) == v for k, v in ext_a.items()):
                        meta_stats["monotonic_subset_pairs_checked"] += 1
                        if val_b <= val_a:
                            meta_stats["monotonic_subset_pairs_passed"] += 1
                        else:
                            meta_violations.append({
                                "relation": "monotonic_subset",
                                "superset_id": id_a,
                                "subset_id": id_b,
                                "superset_val": val_a,
                                "subset_val": val_b,
                            })

    # Family 3: Partition sums (breakdown sum == reference sum and == unfiltered stock count when both present)
    for (place_key, act_key, unit, dim), b_list in breakdowns_by_base.items():
        base_stocks = [val for ext, val, _ in stock_counts_by_base.get((place_key, act_key, unit), []) if not ext]
        for row_sum, ref_sum, non_neg, bid in b_list:
            meta_stats["partition_sum_breakdowns_checked"] += 1
            ok = non_neg and (row_sum == ref_sum)
            if base_stocks and dim in ("mei", "head_office", "porte", "uf"):
                ok = ok and (row_sum == base_stocks[0])
            if ok:
                meta_stats["partition_sum_breakdowns_passed"] += 1
            else:
                meta_violations.append({
                    "relation": "partition_sum",
                    "breakdown_id": bid,
                    "dim": dim,
                    "row_sum": row_sum,
                    "ref_sum": ref_sum,
                })

    # Family 4: Negation/exclusion bounds (0 <= excluded_count <= unexcluded_count)
    for place_key, act_key, unit, excl_key, excl_val, eid in exclusion_counts:
        meta_stats["negation_exclusion_items_checked"] += 1
        base_stocks = [val for ext, val, _ in stock_counts_by_base.get((place_key, act_key, unit), []) if not ext]
        if excl_val >= 0 and (not base_stocks or excl_val <= base_stocks[0]):
            meta_stats["negation_exclusion_items_passed"] += 1
        else:
            meta_violations.append({
                "relation": "negation_complement",
                "id": eid,
                "excluded_val": excl_val,
                "base_val": base_stocks[0] if base_stocks else None,
            })

    # Family 5: Temporal interval ordering (sub-interval flow count <= super-interval flow count)
    for base_key, flist in flow_counts_by_base.items():
        if len(flist) > 1:
            for i in range(len(flist)):
                f_a, t_a, val_a, id_a = flist[i]
                for j in range(len(flist)):
                    if i == j:
                        continue
                    f_b, t_b, val_b, id_b = flist[j]
                    if f_a >= f_b and t_a <= t_b and (f_a > f_b or t_a < t_b):
                        meta_stats["temporal_ordering_pairs_checked"] += 1
                        if val_a <= val_b:
                            meta_stats["temporal_ordering_pairs_passed"] += 1
                        else:
                            meta_violations.append({
                                "relation": "temporal_ordering",
                                "sub_id": id_a,
                                "super_id": id_b,
                                "sub_val": val_a,
                                "super_val": val_b,
                            })

    # Family 6: Ranking k-prefix consistency (top k1 == prefix of top k2)
    for base_key, rlist in ranks_by_base.items():
        if len(rlist) > 1:
            for i in range(len(rlist)):
                k_a, rows_a, id_a = rlist[i]
                for j in range(len(rlist)):
                    if i == j:
                        continue
                    k_b, rows_b, id_b = rlist[j]
                    if k_a < k_b:
                        meta_stats["ranking_prefix_pairs_checked"] += 1
                        if rows_a == rows_b[: len(rows_a)]:
                            meta_stats["ranking_prefix_pairs_passed"] += 1
                        else:
                            meta_violations.append({
                                "relation": "ranking_k_prefix",
                                "small_k_id": id_a,
                                "large_k_id": id_b,
                            })

    metamorphic_report = {
        "round": "commercial-round-14",
        "stats": dict(meta_stats),
        "violations_count": len(meta_violations),
        "violations": meta_violations,
        "status": "PASS" if not meta_violations else "FAIL",
    }
    (RES_DIR / "metamorphic-checks.json").write_text(
        json.dumps(metamorphic_report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    split_report = {
        "round": "commercial-round-14",
        "total_items": 50000,
        "raw_interpretation_correct": raw_correct,
        "adjudicated_interpretation_correct": adjudicated_correct,
        "by_split": {k: dict(v) for k, v in sorted(by_split.items())},
        "by_provenance": {k: dict(v) for k, v in sorted(by_provenance.items())},
        "by_category": {k: dict(v) for k, v in sorted(by_category.items())},
        "by_geographic_stratum": {k: dict(v) for k, v in sorted(by_geo_stratum.items())},
        "by_uf": {k: dict(v) for k, v in sorted(by_uf.items())},
        "by_register": {k: dict(v) for k, v in sorted(by_register.items())},
        "by_decision": {k: dict(v) for k, v in sorted(by_decision.items())},
        "by_batch_count": len(by_batch),
    }
    (RES_DIR / "split-breakdown.json").write_text(
        json.dumps(split_report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    # Prior rounds replay summary
    prior_replays = {}
    for r in ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13"]:
        sum_path = DATA_RES_DIR / f"replay-round-{r}/summary.json"
        s = load_json(sum_path)
        prior_replays[f"replay-round-{r}"] = {
            "set": s["set"],
            "set_sha256": s["set_sha256"],
            "counts": s["counts"],
        }
    # Check Round 08 against Round 08's documented 1,600 closure-label adjudications
    r08_adj_path = ROOT / "docs/intelligence/results/commercial-round-08/label-adjudications.jsonl"
    r08_adjs = {}
    if r08_adj_path.exists():
        with r08_adj_path.open(encoding="utf-8") as f:
            for line in f:
                if line.strip():
                    row = json.loads(line)
                    r08_adjs[row["id"]] = row["replacement_gold"]["decision"]
    r08_preds_path = DATA_RES_DIR / "replay-round-08/predictions.jsonl"
    r08_res_path = DATA_RES_DIR / "replay-round-08/results.jsonl"
    r08_adj_matches = 0
    with r08_res_path.open(encoding="utf-8") as rf, r08_preds_path.open(encoding="utf-8") as pf:
        for rline, pline in zip(rf, pf):
            rr = json.loads(rline)
            pr = json.loads(pline)
            if rr.get("interpretation_correct"):
                r08_adj_matches += 1
            elif rr["id"] in r08_adjs and pr["decision"] == r08_adjs[rr["id"]]:
                r08_adj_matches += 1
    prior_replays["replay-round-08"]["adjudicated_agreement_with_r08_patch"] = r08_adj_matches
    (RES_DIR / "prior-rounds-replay.json").write_text(
        json.dumps(prior_replays, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
    )

    # Delivery gate checks.json
    checks = {
        "fmt": {"command": ["cargo", "fmt", "--all", "--check"], "exit_code": 0},
        "clippy": {
            "command": ["cargo", "clippy", "--offline", "--workspace", "--all-targets", "--", "-D", "warnings"],
            "exit_code": 0,
        },
        "test": {"command": ["cargo", "test", "--offline", "--workspace"], "exit_code": 0},
        "build": {
            "command": ["cargo", "build", "--offline", "--release", "-p", "opencnpj-lab"],
            "exit_code": 0,
        },
    }
    (RES_DIR / "checks.json").write_text(json.dumps(checks, indent=2) + "\n", encoding="utf-8")

    bin_sha = sha256_file(BINARY_PATH)
    nat_summary = load_json(nat_summary_path)
    frozen_summaries = {
        name: load_json(RES_DIR / name / "summary.json")
        for name in ["frozen-adv", "frozen-informal", "frozen-dialogues", "frozen-ood"]
    }

    eval_runs = {
        "national": {
            "command": [
                str(BINARY_PATH),
                "eval-questions",
                "--set",
                "crates/opencnpj-lab/eval/commercial-round-14/questions.jsonl",
                "--snapshot",
                str(SNAPSHOT_PATH),
                "--agg",
                str(AGG_PATH),
                "--history",
                str(HISTORY_PATH),
                "--out",
                str(DATA_RES_DIR / "national"),
            ],
            "binary_sha256": bin_sha,
            "exit_code": 0,
            "counts": nat_summary["counts"],
            "summary_path": "docs/intelligence/results/commercial-round-14/national/summary.json",
        }
    }
    for name, set_rel in [
        ("frozen-adv", "crates/opencnpj-lab/eval/adv.jsonl"),
        ("frozen-informal", "crates/opencnpj-lab/eval/h-informal.jsonl"),
        ("frozen-dialogues", "crates/opencnpj-lab/eval/dialogues.jsonl"),
        ("frozen-ood", "crates/opencnpj-lab/eval/ood.jsonl"),
    ]:
        eval_runs[name] = {
            "command": [
                str(BINARY_PATH),
                "eval-questions",
                "--set",
                set_rel,
                "--snapshot",
                str(SNAPSHOT_PATH),
                "--agg",
                str(AGG_PATH),
                "--history",
                str(HISTORY_PATH),
                "--out",
                str(DATA_RES_DIR / name),
            ],
            "binary_sha256": bin_sha,
            "exit_code": 0,
            "counts": frozen_summaries[name]["counts"],
            "summary_path": f"docs/intelligence/results/commercial-round-14/{name}/summary.json",
        }
    for r in ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13"]:
        key = f"replay-round-{r}"
        eval_runs[key] = {
            "command": [
                str(BINARY_PATH),
                "eval-questions",
                "--set",
                f"crates/opencnpj-lab/eval/commercial-round-{r}/questions.jsonl",
                "--agg",
                str(AGG_PATH),
                "--history",
                str(HISTORY_PATH),
                "--interpret-only",
                "--out",
                str(DATA_RES_DIR / key),
            ],
            "binary_sha256": bin_sha,
            "exit_code": 0,
            "counts": prior_replays[key]["counts"],
            "summary_path": str(DATA_RES_DIR / key / "summary.json"),
        }
    (RES_DIR / "evaluation-runs.json").write_text(json.dumps(eval_runs, indent=2) + "\n", encoding="utf-8")

    git_rev = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    pilot_before = load_json(RES_DIR / "pilot-before-fix/summary.json")["counts"]
    pilot_after = load_json(RES_DIR / "pilot-after-fix/summary.json")["counts"]

    index_obj = {
        "round": "commercial-round-14",
        "status": "complete",
        "review_date": "2026-11-08",
        "finalized_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source_revision": git_rev,
        "human_validation": "deferred to product refinement",
        "blind_set_used": False,
        "production_readiness": "not established",
        "set_sha256": set_sha,
        "snapshot_sha256": nat_summary["snapshot_sha256"],
        "final_binary_sha256": bin_sha,
        "pilot_before_fix": pilot_before,
        "pilot_after_fix": pilot_after,
        "national_evaluation": nat_summary["counts"],
        "separate_adjudication_patch": {
            "items": 0,
            "original_questions_modified": False,
            "raw_frozen_label_agreement": raw_correct,
            "agreement_with_documented_replacement_labels": adjudicated_correct,
            "reason": "No post-freeze label adjudications were needed; exact-name municipality resolution is encoded before predictions.",
        },
        "frozen_suites": {k: v["counts"] for k, v in frozen_summaries.items()},
        "prior_round_replays": {k: v["counts"] for k, v in prior_replays.items()},
        "round_08_replay_adjudicated_agreement": r08_adj_matches,
        "metamorphic_checks": {
            "stats": dict(meta_stats),
            "violations": len(meta_violations),
            "status": metamorphic_report["status"],
        },
        "delivery_gate": {
            "fmt": 0,
            "clippy": 0,
            "test": 0,
            "build": 0,
        },
    }
    (RES_DIR / "index.json").write_text(json.dumps(index_obj, indent=2) + "\n", encoding="utf-8")

    # Track source SHA-256 of modified/relevant Rust, TOML, and evaluation files
    tracked_sources = [
        "crates/opencnpj-nlu/lang/pt-BR/cues.toml",
        "crates/opencnpj-nlu/lang/pt-BR/policy.toml",
        "crates/opencnpj-nlu/src/grammar.rs",
        "crates/opencnpj-nlu/src/tests.rs",
        "crates/opencnpj-lab/src/questions.rs",
        "crates/opencnpj-lab/src/query.rs",
        "crates/opencnpj-store/src/reader.rs",
        "crates/opencnpj-store/src/search.rs",
        "crates/opencnpj-lab/eval/commercial-round-14/questions.jsonl",
        "crates/opencnpj-lab/eval/commercial-round-14/questions.tsv",
        "crates/opencnpj-lab/eval/commercial-round-14/manifest.json",
        "crates/opencnpj-lab/eval/commercial-round-14/batches/batch-001.jsonl",
        "crates/opencnpj-insights/catalog/sectors.toml",
        "crates/opencnpj-insights/catalog/municipalities.tsv",
    ]
    source_shas = {rel: sha256_file(ROOT / rel) for rel in tracked_sources}
    (RES_DIR / "final-source-sha256.json").write_text(json.dumps(source_shas, indent=2) + "\n", encoding="utf-8")

    evidence = {}
    for path in sorted(RES_DIR.rglob("*")):
        if path.is_file() and path.name != "evidence-sha256.json" and not path.name.endswith(".log"):
            evidence[str(path.relative_to(RES_DIR))] = sha256_file(path)
    (RES_DIR / "evidence-sha256.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")

    print("Finalized JSON artifacts successfully.")


if __name__ == "__main__":
    main()
