#!/usr/bin/env python3
"""Author the Skills Company 01 evaluation set.

Questions that look up a company by name ("encontre a empresa Zaflas", "zaflas",
"qual o CNPJ da Padaria Pão Quente em Cuiabá/MT?") with gold CompanySearch or gold IR
built from templates, never from the interpreter. Covers openers × company names,
openers × company names × places (municipalities and UFs), bare company names, regional
registers across Brazil, privacy/unavailable refusals (owner, CPF, revenue) and
activity searches ("busque padarias em X") that must remain record lists.

All items are synthetic. Run from anywhere; output is deterministic.
"""

from __future__ import annotations

import hashlib
import itertools
import json
import tomllib
import unicodedata
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/skills-company-01"
CATALOG = ROOT / "crates/opencnpj-insights/catalog/municipalities.tsv"
SECTORS_PATH = ROOT / "crates/opencnpj-insights/catalog/sectors.toml"

ANCHOR_DAY = 46305  # 2026-10-11
CURRENT_COMP = 321  # 2026-10
VERSIONS = {"municipalities": 202610, "sectors": 1}
UFS = [
    "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA",
    "PB", "PE", "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO",
]


def fold(text: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFD", text) if unicodedata.category(c) != "Mn").lower()


def load():
    municipalities = {}
    with CATALOG.open(encoding="utf-8") as f:
        for line in f.read().splitlines()[1:]:
            cols = line.split("\t")
            municipalities[(cols[4], cols[3])] = int(cols[0])
    sectors = {}
    for i, s in enumerate(tomllib.load(SECTORS_PATH.open("rb"))["sector"]):
        sectors[s["id"]] = {"id": i, "cnaes": [{"subclass": int(c)} for c in sorted(set(s["cnae_core"]))]}
    return municipalities, sectors


MUNIS, SECTOR_IDS = load()

COMPANIES = [
    "Zaflas",
    "Zaflas Confecções Ltda",
    "Mitel Teleinformática",
    "Padaria Pão Quente",
    "Construtora Horizonte Azul",
    "Cerealista Grão Fino",
    "Metalúrgica São Jorge",
    "Laticínios Vale Verde",
    "Gráfica Novo Tempo",
    "Auto Peças Dois Irmãos",
    "Transportadora Rápido Norte",
    " Farmácia Santa Clara".strip(),
]

OPENERS = [
    "Encontre a empresa",
    "Ache a empresa",
    "Busque a empresa",
    "Procure a firma",
    "Pesquise a empresa",
    "Localize o estabelecimento",
    "Consulte a razão social",
    "Verifique a empresa",
    "Quero saber sobre a empresa",
    "Me fale sobre a empresa",
    "Quem é a empresa",
    "Informações sobre a empresa",
    "Dados da empresa",
    "Ficha da empresa",
    "Cadastro da empresa",
    "Qual o CNPJ da",
    "Qual é o CNPJ da empresa",
    "Situação da empresa",
    "Endereço da empresa",
    "Existe a empresa",
]

PLACES = [
    ("em Cuiabá/MT", [MUNIS[("Cuiabá", "MT")]], []),
    ("em Sorriso", [MUNIS[("Sorriso", "MT")]], []),
    ("em Sinop/MT", [MUNIS[("Sinop", "MT")]], []),
    ("em Rondonópolis/MT", [MUNIS[("Rondonópolis", "MT")]], []),
    ("em Várzea Grande/MT", [MUNIS[("Várzea Grande", "MT")]], []),
    ("em Goiânia/GO", [MUNIS[("Goiânia", "GO")]], []),
    ("em Campo Grande/MS", [MUNIS[("Campo Grande", "MS")]], []),
    ("em Londrina/PR", [MUNIS[("Londrina", "PR")]], []),
    ("em Curitiba/PR", [MUNIS[("Curitiba", "PR")]], []),
    ("em Manaus/AM", [MUNIS[("Manaus", "AM")]], []),
    ("em MT", [], [UFS.index("MT")]),
    ("em Mato Grosso", [], [UFS.index("MT")]),
    ("em GO", [], [UFS.index("GO")]),
    ("no Paraná", [], [UFS.index("PR")]),
]

BARE_NAMES = [
    "zaflas",
    "Zaflas Confecções",
    "Zaflas Confecções Ltda",
    "Mitel Teleinformática",
    "Padaria Pão Quente",
    "Construtora Horizonte Azul",
    "Cerealista Grão Fino",
    "Metalúrgica São Jorge",
    "Laticínios Vale Verde",
    "Gráfica Novo Tempo",
    "Transportadora Rápido",
    "Farmácia Santa Clara",
]

BARE_SHORT = [
    "zaflas",
    "Zaflas Confecções",
    "Mitel Teleinformática",
    "Metalúrgica Alpha",
    "Cerealista Primor",
    "Transportadora Rápido",
]

REGIONAL_OPENERS = [
    ("Bah, tchê, encontre a empresa", "sul"),
    ("Barbaridade, guri, qual o CNPJ da empresa", "sul"),
    ("Tchê, me fala sobre a firma", "sul"),
    ("Oxe, ache a empresa", "nordeste"),
    ("Oxente, procure a empresa", "nordeste"),
    ("Meu rei, dados da empresa", "nordeste"),
    ("Uai sô, cê pode buscar a empresa", "mineiro"),
    ("Uai, me fala sobre a empresa", "mineiro"),
    ("Vôte, encontre a empresa", "centro_oeste"),
    ("Tchá por Deus, qual o CNPJ da", "centro_oeste"),
    ("Égua, maninho, localize a empresa", "norte"),
    ("Pai d'égua, pesquise a firma", "norte"),
    ("Mano, busca a empresa", "sudeste"),
    ("Mermão, ficha da empresa", "sudeste"),
    ("Parceiro, dados da firma", "sudeste"),
]

REGIONAL_TAILS = ["", ", visse?", ", por favor", " aí", ", valeu"]

ACTIVITY_VERBS = ["Busque", "Encontre", "Procure", "Pesquise", "Localize", "Ache"]
ACTIVITY_SECTORS = [
    ("bakery", "padarias"),
    ("pharmacy", "farmácias"),
    ("restaurant", "restaurantes"),
    ("gym", "academias"),
    ("pet_shop", "pet shops"),
    ("supermarket", "supermercados"),
]
ACTIVITY_CITIES = [
    ("Cuiabá/MT", ("Cuiabá", "MT")),
    ("Sorriso", ("Sorriso", "MT")),
    ("Sinop/MT", ("Sinop", "MT")),
    ("Rondonópolis/MT", ("Rondonópolis", "MT")),
    ("Goiânia/GO", ("Goiânia", "GO")),
    ("Londrina/PR", ("Londrina", "PR")),
]


def company_gold(name: str, municipalities: list[int] | None = None, ufs: list[int] | None = None) -> dict:
    out: dict = {"name": name}
    if municipalities:
        out["municipalities"] = municipalities
    if ufs:
        out["ufs"] = ufs
    return out


def list_ir(place: dict, sector_key: str) -> dict:
    return {
        "op": {"kind": "list", "order": "cnpj", "limit": 20},
        "unit": "establishments",
        "scope": {
            "place": place,
            "activity": {
                "sectors": {
                    "ids": [SECTOR_IDS[sector_key]["id"]],
                    "cnaes": SECTOR_IDS[sector_key]["cnaes"],
                    "scope": "main",
                }
            },
            "status": ["active"],
        },
        "time": {"competence": CURRENT_COMP, "period": None, "anchor": ANCHOR_DAY},
        "output": {"page": 1},
        "versions": VERSIONS,
    }


def main():
    rows = []
    seen = set()

    def add(item_id: str, text: str, decision: str, family: str, company=None, ir=None, act=None):
        key = fold(text)
        if key in seen:
            return
        seen.add(key)
        gold = {"decision": decision, "ir": ir, "export": None, "company": company}
        if act:
            gold["act"] = act
        rows.append({
            "id": item_id,
            "text": text,
            "gold": gold,
            "meta": {"category": family.split("/")[0], "family": family, "human_validated": False},
        })

    # 1. Openers × company names (240 items)
    idx = 0
    for opener, name in itertools.product(OPENERS, COMPANIES):
        punct = "?" if opener.lower().startswith(("qual", "quem", "existe")) else ""
        add(f"skills-company-01-o{idx:04d}", f"{opener} {name}{punct}", "execute", "opener/direct", company=company_gold(name), act="company_search")
        idx += 1

    # 2. Openers × company names × places (480 items)
    idx = 0
    for (opener, name), (place_text, munis, ufs) in itertools.product(itertools.product(OPENERS[:10], COMPANIES[:7]), PLACES[:4]):
        punct = "?" if opener.lower().startswith(("qual", "quem", "existe")) else ""
        kind = "uf" if ufs else "municipality"
        add(
            f"skills-company-01-p{idx:04d}",
            f"{opener} {name} {place_text}{punct}",
            "execute",
            f"place/{kind}",
            company=company_gold(name, munis, ufs),
            act="company_search",
        )
        idx += 1
    for opener, name, (place_text, munis, ufs) in itertools.product(OPENERS[10:15], COMPANIES[:4], PLACES[4:]):
        punct = "?" if opener.lower().startswith(("qual", "quem", "existe")) else ""
        kind = "uf" if ufs else "municipality"
        add(
            f"skills-company-01-p{idx:04d}",
            f"{opener} {name} {place_text}{punct}",
            "execute",
            f"place/{kind}",
            company=company_gold(name, munis, ufs),
            act="company_search",
        )
        idx += 1

    # 3. Bare company names, with and without place suffix (36 items)
    idx = 0
    for name in BARE_NAMES:
        add(f"skills-company-01-b{idx:04d}", name, "execute", "bare/name", company=company_gold(name), act="company_search")
        idx += 1
    for name, (place_text, munis, ufs) in itertools.product(BARE_SHORT, PLACES[:4]):
        add(
            f"skills-company-01-b{idx:04d}",
            f"{name} {place_text}",
            "execute",
            "bare/place",
            company=company_gold(name, munis, ufs),
            act="company_search",
        )
        idx += 1

    # 4. Regional registers across Brazil (180 items)
    idx = 0
    for n, ((opener, region), name) in enumerate(itertools.product(REGIONAL_OPENERS, COMPANIES)):
        tail = REGIONAL_TAILS[n % len(REGIONAL_TAILS)]
        if n % 3 == 0:
            place_text, munis, ufs = PLACES[n % len(PLACES)]
            text = f"{opener} {name} {place_text}{tail}"
            gold = company_gold(name, munis, ufs)
        else:
            text = f"{opener} {name}{tail}"
            gold = company_gold(name)
        add(f"skills-company-01-r{idx:04d}", text, "execute", f"regional/{region}", company=gold, act="company_search")
        idx += 1

    # 5. Activity searches ("busque padarias em X") stay record lists (72 items)
    idx = 0
    for verb, (sector_key, plural), (place_label, (city, uf_code)) in itertools.product(ACTIVITY_VERBS[:2], ACTIVITY_SECTORS, ACTIVITY_CITIES):
        place = {"municipalities": [MUNIS[(city, uf_code)]]}
        add(
            f"skills-company-01-l{idx:04d}",
            f"{verb} {plural} em {place_label}",
            "execute",
            "activity_list/municipality",
            ir=list_ir(place, sector_key),
            act="new_query",
        )
        idx += 1

    # 6. Privacy and unavailable-metric refusals (60 items)
    refusal_templates = [
        ("Encontre o dono da empresa {name}", "refusal/privacy"),
        ("Quem são os sócios da {name}?", "refusal/privacy"),
        ("Qual o CPF do dono da {name}?", "refusal/privacy"),
        ("Me fale o quadro societário da empresa {name}", "refusal/privacy"),
        ("Qual o faturamento da empresa {name}?", "refusal/unavailable"),
        ("Qual a receita bruta da empresa {name}?", "refusal/unavailable"),
        ("Qual o lucro da empresa {name}?", "refusal/unavailable"),
        ("Quantos funcionários tem a empresa {name}?", "refusal/unavailable"),
    ]
    idx = 0
    for (tpl, fam), name in itertools.product(refusal_templates, COMPANIES[:7]):
        add(f"skills-company-01-s{idx:04d}", tpl.format(name=name), "reject", fam)
        idx += 1
    for greeting in ["Bom dia", "Boa tarde, obrigado", "Oi, por favor", "Consulte o CNPJ 33.000.167/0001-00"]:
        add(f"skills-company-01-s{idx:04d}", greeting, "reject" if "CNPJ" not in greeting else "clarify", "refusal/stop")
        idx += 1

    lines = "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows)
    EVAL_DIR.mkdir(parents=True, exist_ok=True)
    path = EVAL_DIR / "questions.jsonl"
    path.write_text(lines, encoding="utf-8")
    families = {}
    for row in rows:
        families[row["meta"]["family"]] = families.get(row["meta"]["family"], 0) + 1
    manifest = {
        "set": "skills-company-01",
        "items": len(rows),
        "sha256": hashlib.sha256(lines.encode()).hexdigest(),
        "families": dict(sorted(families.items())),
        "anchor": "2026-10-11",
        "generator": "author_skills_company.py",
    }
    (EVAL_DIR / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
