#!/usr/bin/env python3
"""Author the Prospecting 01 evaluation set.

Questions written the way people look for customers: search-style noun phrases without a
verb ("dentistas em Cuiabá com telefone"), contact and lead requests, new companies,
capital, age, size and legal form, coordinated places, and requests that must be asked
about instead of executed ("empresas de energia solar", "com DDD 66"). Gold labels come
from templates, never from the interpreter.

All items are synthetic. Run from anywhere; output is deterministic.
"""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/prospecting-01"
spec = importlib.util.spec_from_file_location("export", ROOT / "crates/opencnpj-lab/eval/skills-export-01/author_skills_export.py")
ref = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(ref)
ir, muni, uf, day, year_range = ref.ir, ref.muni, ref.uf, ref.day, ref.year_range
ALL_STATUS, ANCHOR, SECTOR_IDS, MUNIS = ref.ALL_STATUS, ref.ANCHOR_DAY, ref.SECTOR_IDS, ref.MUNIS

COUNT = {"kind": "count", "metric": "stock"}
OPENED = {"kind": "count", "metric": {"flow": "registrations"}}
LIST = {"kind": "list", "order": "cnpj", "limit": 20}
BIGGEST = {"kind": "list", "order": "capital_desc", "limit": 20}
ACTIVE = ["active"]

# (sector key, plural surface, feminine)
SECTORS = [
    ("dental_clinic", "dentistas", False),
    ("pharmacy", "farmácias", True),
    ("restaurant", "restaurantes", False),
    ("gym", "academias", True),
    ("law_firm", "advogados", False),
    ("real_estate_broker", "imobiliárias", True),
    ("truck_freight", "transportadoras", True),
    ("accounting", "escritórios de contabilidade", False),
    ("driving_school", "autoescolas", True),
    ("medical_practice", "clínicas médicas", True),
]
# Municipality names that exist once in the country, so no state is needed.
PLACES = [("Cuiabá", "MT"), ("Sinop", "MT"), ("Rondonópolis", "MT"), ("Sorriso", "MT"), ("Goiânia", "GO"), ("Londrina", "PR")]


def any_ir(op, place, status, extra=None, period=None):
    """Request over every activity."""
    request = ir(op, place, "bakery", status, extra, period)
    del request["scope"]["activity"]
    return request


def days(first: int, last: int) -> dict:
    return {"days": {"from": first, "to": last}}


def adj(feminine: bool, word: str) -> str:
    return word + ("as" if feminine else "os")


def items():
    """(family, text, decision, gold IR, gold company)."""
    for (key, plural, fem), (city, code) in ((s, p) for s in SECTORS for p in PLACES):
        place = muni(city, code)
        art = "as" if fem else "os"
        # A noun phrase is a complete search: the stock of active establishments.
        yield "search/topic_status", f"{plural} {adj(fem, 'ativ')} em {city}/{code}", "execute", ir(COUNT, place, key, ACTIVE), None
        yield "search/request_led", f"quero {plural} em {city}/{code}", "execute_with_note", ir(COUNT, place, key, ACTIVE), None
        yield "search/request_led", f"procuro {plural} em {city} {code}", "execute_with_note", ir(COUNT, place, key, ACTIVE), None
        yield "search/competitors", f"concorrentes de {plural} em {city}/{code}", "execute_with_note", ir(COUNT, place, key, ACTIVE), None
        # Events and periods without a verb.
        yield "search/opened_year", f"{plural} {adj(fem, 'abert')} em 2024 em {city}/{code}", "execute", ir(OPENED, place, key, ALL_STATUS, None, days(day(2024, 1, 1), day(2024, 12, 31))), None
        yield "search/opened_recent", f"{plural} {adj(fem, 'abert')} nos últimos 30 dias em {city}/{code}", "execute", ir(OPENED, place, key, ALL_STATUS, None, days(ANCHOR - 29, ANCHOR)), None
        yield "search/new", f"{adj(fem, 'nov')} {plural} em {city}/{code}", "execute_with_note", ir(OPENED, place, key, ALL_STATUS, None, days(ANCHOR - 89, ANCHOR)), None
        # Contacts and leads are lists of companies.
        yield "contacts/phones_of", f"telefones de {plural} em {city}/{code}", "execute", ir(LIST, place, key, ACTIVE, {"contact": "phone"}), None
        yield "contacts/emails_of", f"emails de {plural} em {city}/{code}", "execute", ir(LIST, place, key, ACTIVE, {"contact": "email"}), None
        yield "contacts/with_phone", f"{plural} em {city}/{code} com telefone", "execute", ir(LIST, place, key, ACTIVE, {"contact": "phone"}), None
        yield "contacts/with_both", f"{plural} {adj(fem, 'ativ')} em {city}/{code} com telefone e email", "execute", ir(LIST, place, key, ACTIVE, {"contact": "both"}), None
        yield "contacts/whatsapp", f"liste {art} {plural} de {city}/{code} com whatsapp", "execute_with_note", ir(LIST, place, key, ACTIVE, {"contact": "phone"}), None
        yield "contacts/contacts_of", f"me dá os contatos {'das' if fem else 'dos'} {plural} de {city}/{code}", "execute", ir(LIST, place, key, ACTIVE), None
        yield "contacts/leads", f"leads de {plural} em {city}/{code}", "execute", ir(LIST, place, key, ACTIVE), None
        # Capital without the word "social".
        yield "capital/above", f"liste {art} {plural} com capital acima de 100 mil em {city}/{code}", "execute", ir(LIST, place, key, ACTIVE, {"capital": {"from": 10_000_001, "to": None}}), None
        yield "capital/at_least", f"{plural} em {city}/{code} com capital de pelo menos 50 mil", "execute_with_note", ir(COUNT, place, key, ACTIVE, {"capital": {"from": 5_000_000, "to": None}}), None
        yield "capital/up_to", f"liste {art} {plural} com capital até 10 mil em {city}/{code}", "execute", ir(LIST, place, key, ACTIVE, {"capital": {"from": None, "to": 1_000_000}}), None
        # The only size the registry holds is the declared capital.
        yield "order/biggest", f"liste {art} maiores {plural} de {city}/{code}", "execute_with_note", ir(BIGGEST, place, key, ACTIVE), None
        # Words no catalogue or filter holds are asked about, never dropped.
        yield "unknown/filter_word", f"liste {art} {plural} de {city}/{code} com site", "clarify", None, None
        yield "unknown/area_code", f"liste {art} {plural} de {city}/{code} com DDD 66", "clarify", None, None
        yield "unknown/neighbourhood", f"liste {art} {plural} do bairro Centro em {city}/{code}", "clarify", None, None
    for city, code in PLACES:
        place = muni(city, code)
        yield "search/companies", f"empresas em {city}/{code}", "execute_with_note", any_ir(COUNT, place, ACTIVE), None
        yield "search/companies_mei", f"MEIs ativos em {city}/{code}", "execute", any_ir(COUNT, place, ACTIVE, {"mei": "yes"}), None
        yield "search/companies_simples", f"empresas do simples nacional em {city}/{code}", "execute_with_note", any_ir(COUNT, place, ACTIVE, {"simples": "yes"}), None
        yield "search/companies_status", f"empresas inaptas em {city}/{code}", "execute", any_ir(COUNT, place, ["unfit"]), None
        yield "search/opened_recent", f"empresas abertas nos últimos 30 dias em {city}/{code}", "execute", any_ir(OPENED, place, ALL_STATUS, None, days(ANCHOR - 29, ANCHOR)), None
        yield "search/new", f"novas empresas em {city}/{code}", "execute_with_note", any_ir(OPENED, place, ALL_STATUS, None, days(ANCHOR - 89, ANCHOR)), None
        yield "period/this_month", f"liste as empresas abertas este mês em {city}/{code}", "execute_with_note", any_ir(LIST, place, ALL_STATUS, {"opened": {"from": day(2026, 10, 1), "to": ANCHOR}}), None
        yield "period/this_week", f"liste as empresas abertas esta semana em {city}/{code}", "execute_with_note", any_ir(LIST, place, ALL_STATUS, {"opened": {"from": ANCHOR - 6, "to": ANCHOR}}), None
        yield "age/older", f"liste as empresas com mais de 10 anos em {city}/{code}", "execute_with_note", any_ir(LIST, place, ACTIVE, {"opened": {"from": None, "to": day(2016, 10, 11)}}), None
        yield "age/younger", f"empresas com menos de 2 anos em {city}/{code}", "execute_with_note", any_ir(COUNT, place, ACTIVE, {"opened": {"from": day(2024, 10, 11) + 1, "to": ANCHOR}}), None
        yield "size/medium", f"liste empresas de médio porte em {city}/{code}", "execute_with_note", any_ir(LIST, place, ACTIVE, {"porte": ["other"]}), None
        yield "size/large", f"empresas de grande porte em {city}/{code}", "execute_with_note", any_ir(COUNT, place, ACTIVE, {"porte": ["other"]}), None
        yield "legal_form/cooperatives", f"liste as cooperativas de {city}/{code}", "execute", any_ir(LIST, place, ACTIVE, {"natureza": [{"code": 2143}]}), None
        yield "legal_form/corporations", f"sociedades anônimas em {city}/{code}", "execute_with_note", any_ir(COUNT, place, ACTIVE, {"natureza": [{"code": 2046}, {"code": 2054}]}), None
        yield "broad/wholesale", f"quantos atacadistas existem em {city}/{code}", "execute", ir(COUNT, place, "wholesale", ACTIVE), None
        yield "broad/manufacturing", f"indústrias em {city}/{code}", "execute_with_note", ir(COUNT, place, "manufacturing", ACTIVE), None
        yield "prospecting/who_sells", f"quem vende material de construção em {city}/{code}", "execute", ir(LIST, place, "building_materials", ACTIVE), None
        yield "prospecting/sell_to_new", f"quero vender para restaurantes novos em {city}/{code}", "execute_with_note", ir(OPENED, place, "restaurant", ALL_STATUS, None, days(ANCHOR - 89, ANCHOR)), None
        # The activity after "para minha" is the asker's own business, not the companies sought.
        yield "prospecting/own_business", f"preciso prospectar clientes para minha distribuidora de bebidas em {city}", "clarify", None, None
        yield "prospecting/no_target", f"quero encontrar clientes potenciais em {city}/{code}", "clarify", None, None
        yield "unknown/activity_list", f"liste as empresas de energia solar em {city}/{code}", "clarify", None, None
        yield "unknown/activity_count", f"quantas empresas de criptomoedas existem em {city}/{code}", "clarify", None, None
        yield "unknown/activity_search", f"empresas de energia solar em {city}/{code}", "clarify", None, None
        yield "unknown/activity_noun", f"liste os fornecedores de embalagens em {city}/{code}", "clarify", None, None
        yield "unknown/number", f"quantas empresas existem em {city}/{code} com 15 funcionarios", "reject", None, None
    # Coordinated places: every named city stays in the query.
    pairs = [("Sinop", "Sorriso"), ("Cuiabá", "Rondonópolis"), ("Sorriso", "Sinop")]
    for (key, plural, fem), (a, b) in ((s, p) for s in SECTORS for p in pairs):
        both = {"municipalities": sorted([MUNIS[(a, "MT")], MUNIS[(b, "MT")]])}
        art = "as" if fem else "os"
        yield "places/coordinated_list", f"liste {art} {plural} de {a} e {b}", "execute", ir(LIST, both, key, ACTIVE), None
        yield "places/coordinated_search", f"{plural} em {a} e {b}", "execute_with_note", ir(COUNT, both, key, ACTIVE), None
    # The establishments of a named company.
    for name in ["Bradesco", "Sicredi", "Havan"]:
        yield "company/branches", f"filiais do {name} em MT", "execute", None, {"name": name, "municipalities": [], "ufs": [ref.UFS.index("MT")]}
        yield "company/units", f"unidades da {name} em Goiás", "execute", None, {"name": name, "municipalities": [], "ufs": [ref.UFS.index("GO")]}


def main():
    rows, seen = [], set()
    for n, (family, text, decision, gold_ir, company) in enumerate(items()):
        assert text not in seen, text
        seen.add(text)
        rows.append({
            "id": f"prospecting-01-{n:04d}",
            "text": text,
            "gold": {"decision": decision, "ir": gold_ir, "export": None, "company": company, "act": "new_query"},
            "meta": {"category": family.split("/")[0], "family": family, "human_validated": False},
        })
    lines = "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows)
    (EVAL_DIR / "questions.jsonl").write_text(lines, encoding="utf-8")
    families = {}
    for r in rows:
        families[r["meta"]["family"]] = families.get(r["meta"]["family"], 0) + 1
    manifest = {"set": "prospecting-01", "items": len(rows), "sha256": hashlib.sha256(lines.encode()).hexdigest(), "families": families}
    (EVAL_DIR / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"items": len(rows), "families": len(families)}))


if __name__ == "__main__":
    main()
