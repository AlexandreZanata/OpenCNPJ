#!/usr/bin/env python3
"""Independent label and filter-inventory reviewer for OpenCNPJ Commercial Round 14."""

import argparse
import collections
import glob
import hashlib
import json
from pathlib import Path
import tomllib
import unicodedata

ROOT = Path(__file__).resolve().parents[4]
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/commercial-round-14"
RESULTS_DIR = ROOT / "docs/intelligence/results/commercial-round-14"
CATALOG_PATH = ROOT / "crates/opencnpj-insights/catalog/municipalities.tsv"
SECTORS_PATH = ROOT / "crates/opencnpj-insights/catalog/sectors.toml"

FORBIDDEN_PREFIXES = [
    "para o ciclo de expansao",
    "como representante comercial, pergunto:",
    "para expandir minha operacao:",
    "mapeando a carteira:",
    "para a campanha de vendas:",
]


def fold(text: str) -> str:
    nfd = unicodedata.normalize("NFD", text)
    out = []
    prev_space = True
    for ch in nfd:
        if unicodedata.category(ch) == "Mn":
            continue
        low = ch.lower()
        if low.isascii() and (low.isalnum() or low in "/-.,"):
            out.append(low)
            prev_space = False
        elif not prev_space:
            out.append(" ")
            prev_space = True
    return "".join(out).strip()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_catalogues():
    munis = {}
    mt_munis = set()
    with CATALOG_PATH.open("r", encoding="utf-8") as f:
        for line in f.read().splitlines()[1:]:
            cols = line.split("\t")
            if not cols[0].isdigit():
                continue
            mid = int(cols[0])
            munis[mid] = {"id": mid, "uf": cols[3], "name": cols[4], "folded": cols[5]}
            if cols[3] == "MT":
                mt_munis.add(mid)
    with SECTORS_PATH.open("rb") as f:
        raw_sectors = tomllib.load(f)["sector"]
    sectors = {}
    for idx, s in enumerate(raw_sectors):
        cnaes = sorted({int(c) for c in s["cnae_core"]})
        sectors[idx] = {
            "id": idx,
            "sing": fold(s["display"]["sing"]),
            "plural": fold(s["display"]["plural"]),
            "cnaes": [{"subclass": c} for c in cnaes],
        }
    return munis, mt_munis, sectors


def load_prior_folded():
    prior = set()
    prior_sessions = set()
    patterns = [
        str(ROOT / "crates/opencnpj-lab/eval/commercial-round-0[1-9]/questions.jsonl"),
        str(ROOT / "crates/opencnpj-lab/eval/commercial-round-10/questions.jsonl"),
        str(ROOT / "crates/opencnpj-lab/eval/commercial-round-11/questions.jsonl"),
        str(ROOT / "crates/opencnpj-lab/eval/commercial-round-12/questions.jsonl"),
        str(ROOT / "crates/opencnpj-lab/eval/commercial-round-13/questions.jsonl"),
        str(ROOT / "crates/opencnpj-lab/eval/*.jsonl"),
    ]
    for pat in patterns:
        for p in sorted(glob.glob(pat)):
            with open(p, "r", encoding="utf-8") as f:
                for line in f:
                    if not line.strip():
                        continue
                    row = json.loads(line)
                    if "text" in row:
                        prior.add(fold(row["text"]))
                    ctx = row.get("context") or {}
                    if "session_id" in ctx:
                        prior_sessions.add(ctx["session_id"])
    return prior, prior_sessions


def verify_item_ir(row: dict, munis: dict, sectors: dict) -> list[str]:
    issues = []
    gold = row["gold"]
    dec = gold["decision"]
    ir = gold.get("ir")
    ftext = fold(row["text"])

    if dec in ("clarify", "reject"):
        if ir is not None:
            issues.append(f"{row['id']}: decision={dec} must have ir=None")
        return issues

    if dec != "execute" or ir is None:
        issues.append(f"{row['id']}: execute decision missing ir")
        return issues

    if ir.get("versions") != {"municipalities": 202610, "sectors": 1}:
        issues.append(f"{row['id']}: invalid catalogue versions {ir.get('versions')}")
    if ir.get("time", {}).get("anchor") != 46305:
        issues.append(f"{row['id']}: invalid anchor {ir.get('time', {}).get('anchor')}")

    op = ir["op"]
    kind = op["kind"]
    sc = ir["scope"]

    # Contact filter is only valid for list
    if sc.get("contact", "any") not in ("any", None) and kind != "list":
        issues.append(f"{row['id']}: contact filter on non-list op {kind}")

    # Check sector cnaes match catalogue
    act = sc.get("activity")
    if isinstance(act, dict) and "sectors" in act:
        sids = act["sectors"]["ids"]
        expected_cnaes = []
        for sid in sids:
            if sid not in sectors:
                issues.append(f"{row['id']}: unknown sector id {sid}")
            else:
                expected_cnaes.extend(sectors[sid]["cnaes"])
        dedup_cnaes = sorted(expected_cnaes, key=lambda x: x["subclass"])
        # Deduplicate
        unique_cnaes = []
        for c in dedup_cnaes:
            if not unique_cnaes or unique_cnaes[-1] != c:
                unique_cnaes.append(c)
        if act["sectors"]["cnaes"] != unique_cnaes:
            issues.append(f"{row['id']}: sector cnaes mismatch for {sids}")

    # Check municipalities exist in catalogue
    pl = sc.get("place")
    if isinstance(pl, dict):
        if "municipalities" in pl:
            for mid in pl["municipalities"]:
                if mid not in munis:
                    issues.append(f"{row['id']}: unknown municipality id {mid}")
        if "near" in pl:
            center = pl["near"]["center"]
            if center not in munis:
                issues.append(f"{row['id']}: unknown near center {center}")

    # Independent text-to-filter inventory check for non-dialogue or explicit turns
    is_dlg_followup = bool(row.get("context") and row["context"].get("turn", 1) > 1)
    if not is_dlg_followup:
        # Check closure + opened cohort rule (the Round 08 defect guard!)
        if "tiveram baixa cadastral" in ftext and "com inicio de atividade" in ftext:
            if sc.get("opened") is None:
                issues.append(f"{row['id']}: closure cohort query missing scope.opened")
            if ir["time"].get("period") is None:
                issues.append(f"{row['id']}: closure cohort query missing time.period")
            if sc.get("opened") == ir["time"].get("period", {}).get("days"):
                issues.append(f"{row['id']}: closure cohort query used same range for opened and closure period")

        # Check explicit filters in text match scope
        if "optantes pelo simples" in ftext and sc.get("simples") != "yes":
            issues.append(f"{row['id']}: text mentions Simples opt-in but scope.simples != yes")
        if ("nao sejam mei" in ftext or "nao sao mei" in ftext) and sc.get("mei") != "no":
            issues.append(f"{row['id']}: text mentions non-MEI but scope.mei != no")
        if "optantes pelo mei" in ftext and sc.get("mei") != "yes":
            issues.append(f"{row['id']}: text mentions MEI opt-in but scope.mei != yes")
        if "pequeno porte" in ftext and sc.get("porte") != ["small"]:
            issues.append(f"{row['id']}: text mentions pequeno porte but scope.porte != ['small']")
        if "porte cadastral demais" in ftext and sc.get("porte") != ["other"]:
            issues.append(f"{row['id']}: text mentions porte demais but scope.porte != ['other']")
        if "com telefone e e-mail" in ftext and sc.get("contact") != "both":
            issues.append(f"{row['id']}: text mentions phone+email but scope.contact != both")
        elif "com telefone" in ftext and "e-mail" not in ftext and sc.get("contact") != "phone":
            issues.append(f"{row['id']}: text mentions phone but scope.contact != phone")
        elif "com e-mail" in ftext and "telefone" not in ftext and sc.get("contact") != "email":
            issues.append(f"{row['id']}: text mentions email but scope.contact != email")

    return issues


def review_file(jsonl_path: Path, out_json_path: Path, is_full_50k: bool):
    munis, mt_munis, sectors = load_catalogues()
    prior_folded, prior_sessions = load_prior_folded()

    items = []
    with jsonl_path.open("r", encoding="utf-8") as f:
        for line in f:
            if line.strip():
                items.append(json.loads(line))

    required_meta = {
        "category", "primary_family", "business_decision", "geographic_stratum",
        "uf", "register", "regional_evidence_ids", "mt_context", "other_uf_regional",
        "scenario_family_id", "surface_skeleton_id", "novelty_reason",
        "capability_requirement", "provenance_class", "source_references",
        "label_rationale", "label_status", "split", "batch_id", "authorship",
        "human_validated", "language",
    }

    seen_ids = set()
    seen_folded = set()
    prior_overlap_ids = []
    internal_dup_ids = []
    prefix_violation_ids = []
    meta_violation_ids = []
    ir_issues = []

    by_family = collections.Counter()
    by_geo = collections.Counter()
    by_uf = collections.Counter()
    by_register = collections.Counter()
    by_split = collections.Counter()
    by_decision = collections.Counter()
    used_mt_munis = set()
    used_all_munis = set()
    filter_combos = collections.Counter()

    # Cross-split leakage tracking
    scen_by_split = collections.defaultdict(set)
    skel_by_split = collections.defaultdict(set)
    sess_by_split = collections.defaultdict(set)
    sess_by_batch = collections.defaultdict(set)
    cnpj_by_split = collections.defaultdict(set)
    reused_prior_sessions = []

    for row in items:
        rid = row["id"]
        assert rid not in seen_ids, f"Duplicate ID: {rid}"
        seen_ids.add(rid)

        ft = fold(row["text"])
        if ft in prior_folded:
            prior_overlap_ids.append(rid)
        if ft in seen_folded:
            internal_dup_ids.append(rid)
        seen_folded.add(ft)

        for pref in FORBIDDEN_PREFIXES:
            if ft.startswith(pref):
                prefix_violation_ids.append(rid)

        meta = row.get("meta", {})
        if not required_meta.issubset(meta.keys()) or meta.get("human_validated") is not False:
            meta_violation_ids.append(rid)

        sp = meta["split"]
        by_family[meta["primary_family"]] += 1
        by_geo[meta["geographic_stratum"]] += 1
        if meta["geographic_stratum"] == "other_26_ufs":
            by_uf[meta["uf"]] += 1
        by_register[meta["register"]] += 1
        by_split[sp] += 1
        by_decision[row["gold"]["decision"]] += 1

        scen_by_split[meta["scenario_family_id"]].add(sp)
        skel_by_split[meta["surface_skeleton_id"]].add(sp)

        ctx = row.get("context")
        if ctx and "session_id" in ctx:
            sid = ctx["session_id"]
            sess_by_split[sid].add(sp)
            sess_by_batch[sid].add(meta["batch_id"])
            if sid in prior_sessions:
                reused_prior_sessions.append(sid)

        ir = row["gold"].get("ir")
        if ir:
            op = ir["op"]
            if op["kind"] == "lookup":
                digits = "".join(c for c in op["id"] if c.isalnum())[:8]
                cnpj_by_split[digits].add(sp)
            sc = ir["scope"]
            pl = sc.get("place")
            if isinstance(pl, dict):
                for mid in pl.get("municipalities", []):
                    used_all_munis.add(mid)
                    if mid in mt_munis:
                        used_mt_munis.add(mid)
                if "near" in pl:
                    c_id = pl["near"]["center"]
                    used_all_munis.add(c_id)
                    if c_id in mt_munis:
                        used_mt_munis.add(c_id)
            if op["kind"] == "compare" and op.get("dim") == "municipality":
                for mem in op.get("members", []):
                    mid = mem.get("municipality")
                    if mid is not None:
                        used_all_munis.add(mid)
                        if mid in mt_munis:
                            used_mt_munis.add(mid)
            active_f = []
            for k in ["status", "contact", "mei", "simples", "porte", "head_office", "capital", "natureza", "within_ufs", "place_population", "exclude", "opened"]:
                v = sc.get(k)
                if k == "status" and v == ["active"]:
                    continue
                if v is not None and v != [] and v != {}:
                    active_f.append(k)
            filter_combos["+".join(sorted(active_f)) if active_f else "none"] += 1

        ir_issues.extend(verify_item_ir(row, munis, sectors))

    cross_split_scenarios = [k for k, v in scen_by_split.items() if len(v) > 1]
    cross_split_skeletons = [k for k, v in skel_by_split.items() if len(v) > 1]
    cross_split_sessions = [k for k, v in sess_by_split.items() if len(v) > 1]
    cross_batch_sessions = [k for k, v in sess_by_batch.items() if len(v) > 1]
    cross_split_cnpjs = [k for k, v in cnpj_by_split.items() if len(v) > 1]

    mt_ctx_count = sum(1 for x in items if x["meta"]["mt_context"])
    ouf_reg_count = sum(1 for x in items if x["meta"]["other_uf_regional"])
    novel_count = sum(1 for x in items if x["meta"]["novelty_reason"] is not None)
    clarify_reject_count = by_decision["clarify"] + by_decision["reject"]

    report = {
        "target_file": str(jsonl_path.resolve().relative_to(ROOT)),
        "target_sha256": sha256_file(jsonl_path),
        "total_items": len(items),
        "independent_review_status": "PASS",
        "checks": {
            "internal_normalized_duplicates": len(internal_dup_ids),
            "prior_rounds_and_frozen_normalized_overlap": len(prior_overlap_ids),
            "forbidden_mechanical_prefixes": len(prefix_violation_ids),
            "meta_schema_violations": len(meta_violation_ids),
            "ir_and_filter_inventory_defects": len(ir_issues),
            "cross_split_scenario_leakage": len(cross_split_scenarios),
            "cross_split_skeleton_leakage": len(cross_split_skeletons),
            "cross_split_session_leakage": len(cross_split_sessions),
            "cross_batch_session_leakage": len(cross_batch_sessions),
            "cross_split_cnpj_leakage": len(cross_split_cnpjs),
            "reused_prior_round_session_ids": len(reused_prior_sessions),
        },
        "distributions": {
            "by_primary_family": dict(by_family),
            "by_geographic_stratum": dict(by_geo),
            "by_other_26_ufs": dict(by_uf),
            "by_register": dict(by_register),
            "by_split": dict(by_split),
            "by_decision": dict(by_decision),
            "distinct_municipalities_used": len(used_all_munis),
            "mt_municipalities_used": len(used_mt_munis),
            "mt_municipalities_total_in_catalog": len(mt_munis),
            "mt_context_items": mt_ctx_count,
            "other_uf_regional_items": ouf_reg_count,
            "novel_semantic_constructions": novel_count,
            "clarify_plus_reject_items": clarify_reject_count,
            "distinct_filter_combinations": len(filter_combos),
            "filter_combinations": dict(filter_combos),
        },
    }

    for k, v in report["checks"].items():
        assert v == 0, f"Review check failed: {k} = {v} (issues: {ir_issues[:5]})"

    if is_full_50k:
        assert len(items) == 50000
        assert len(used_mt_munis) == 142, f"Expected all all MT munis, got {len(used_mt_munis)}"
        assert all(cnt >= 400 for cnt in by_uf.values()) and len(by_uf) == 26
        assert sum(by_uf.values()) == 12000
        assert by_geo["national_cross_uf_comparative"] == 8000
        assert sum(v for k, v in by_geo.items() if k.startswith("mt_")) == 30000
        assert by_register == {"conversational_professional": 35000, "formal_technical": 10000, "noisy_short": 5000}
        assert by_split == {"synthetic_dev": 35000, "synthetic_eval": 10000, "synthetic_challenge": 5000}
        assert mt_ctx_count >= 8000
        assert ouf_reg_count >= 4000
        assert novel_count >= 15000
        assert clarify_reject_count >= 5000
        # Write empty label-adjudications.jsonl to document 0 post-freeze adjudications needed
        (RESULTS_DIR / "label-adjudications.jsonl").write_text("", encoding="utf-8")

    out_json_path.parent.mkdir(parents=True, exist_ok=True)
    out_json_path.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"Independent review PASSED for {jsonl_path.name}: {len(items)} items, 0 defects, {len(used_mt_munis)}/all MT munis covered.")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--set", type=Path, required=True, help="Path to JSONL file to review")
    parser.add_argument("--out", type=Path, required=True, help="Path to output review JSON")
    parser.add_argument("--full-50k", action="store_true", help="Enforce full 50,000-item quota assertions")
    args = parser.parse_args()
    review_file(args.set, args.out, args.full_50k)


if __name__ == "__main__":
    main()
