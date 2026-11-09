#!/usr/bin/env python3
"""Author Probes 01: questions that once produced a silently wrong answer.

Each item records a defect found by an audit (2026-11-09): the answer looked fine but
ignored or inverted part of the question. Gold labels come from templates, never from the
interpreter. Keep every item: a probe is retired only with a documented reason.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/probes-01"
spec = importlib.util.spec_from_file_location("export", ROOT / "crates/opencnpj-lab/eval/skills-export-01/author_skills_export.py")
ref = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(ref)
ir, muni, LIST, export, ALL_STATUS = ref.ir, ref.muni, ref.LIST, ref.export, ref.ALL_STATUS
INACTIVE = ["suspended", "unfit", "closed"]

# (defect, text, decision, gold IR, gold export)
PROBES = []
for city, uf in [("Sorriso", "MT"), ("Cuiabá", "MT"), ("Goiânia", "GO")]:
    place = muni(city, uf)
    PROBES += [
        ("closed_word_read_as_active", f"liste as padarias fechadas em {city}/{uf}", "execute", ir(LIST, place, "bakery", ["closed"]), None),
        ("closed_word_read_as_active", f"quantas farmácias encerradas existem em {city}/{uf}?", "execute",
         ir({"kind": "count", "metric": "stock"}, place, "pharmacy", ["closed"]), None),
        ("inactive_read_as_active", f"quantas padarias inativas existem em {city}/{uf}?", "execute_with_note",
         ir({"kind": "count", "metric": "stock"}, place, "bakery", INACTIVE), None),
        ("legal_form_read_as_status", f"quantas padarias ativas de sociedade anônima fechada existem em {city}/{uf}?", "execute",
         ir({"kind": "count", "metric": "stock"}, place, "bakery", ["active"], {"natureza": [{"code": 2054}]}), None),
        ("list_year_dropped", f"liste as padarias de 2025 em {city}/{uf}", "clarify", None, None),
        ("list_year_dropped", f"liste as farmácias em {city}/{uf} desde 2020", "clarify", None, None),
        ("unknown_kind_dropped", f"quero a lista de empresas xyzolicas de {city}/{uf}", "clarify", None, None),
        ("unknown_kind_dropped", f"Lista de empresas agricolas em {city} {uf.lower()}", "execute",
         ir(LIST, place, "agriculture_livestock", ["active"]), None),
        ("row_count_truncated", f"me mande um excel com as 5000 padarias de {city}/{uf}", "execute",
         ir({"kind": "list", "order": "cnpj", "limit": 100}, place, "bakery", ["active"]), dict(export("xlsx"), rows=5000)),
    ]
PROBES += [
    ("city_before_state_dropped", "quantas padarias tem Belém do Pará?", "execute",
     ir({"kind": "count", "metric": "stock"}, muni("Belém", "PA"), "bakery", ["active"]), None),
]


def main():
    rows = []
    for n, (defect, text, decision, gold_ir, gold_export) in enumerate(PROBES):
        rows.append({
            "id": f"probes-01-{n:04d}",
            "text": text,
            "gold": {"decision": decision, "ir": gold_ir, "export": gold_export, "act": "new_query"},
            "meta": {"category": defect, "family": f"probe/{defect}", "found": "2026-11-09", "human_validated": False},
        })
    lines = "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows)
    (EVAL_DIR / "questions.jsonl").write_text(lines, encoding="utf-8")
    families = {}
    for r in rows:
        families[r["meta"]["category"]] = families.get(r["meta"]["category"], 0) + 1
    manifest = {"set": "probes-01", "items": len(rows), "sha256": hashlib.sha256(lines.encode()).hexdigest(), "families": families}
    (EVAL_DIR / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
