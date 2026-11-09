#!/usr/bin/env python3
"""Author the Skills Export 01 evaluation set.

Questions that ask for a file ("me mande um Excel com…", "em CSV", "apenas o nome e o
número") with gold IR and gold export built from templates, never from the interpreter.
Covers single questions, multi-turn dialogues (follow-ups that reuse the previous
answer) and questions that must be declined or clarified.

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
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/skills-export-01"
CATALOG = ROOT / "crates/opencnpj-insights/catalog/municipalities.tsv"
SECTORS = ROOT / "crates/opencnpj-insights/catalog/sectors.toml"

ANCHOR_DAY = 46305  # 2026-10-11
CURRENT_COMP = 321  # 2026-10
VERSIONS = {"municipalities": 202610, "sectors": 1}
ALL_STATUS = ["null", "active", "suspended", "unfit", "closed"]
UFS = ["AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA", "MG", "MS", "MT", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO", "RR", "RS", "SC", "SE", "SP", "TO"]
UF_NAMES = {"MT": "Mato Grosso", "GO": "Goiás", "MS": "Mato Grosso do Sul", "PR": "Paraná", "SP": "São Paulo"}
ALL_FIELDS = [
    "cnpj", "razao_social", "nome_fantasia", "matriz_filial", "situacao", "data_situacao", "motivo_situacao", "data_inicio",
    "cnae_principal", "cnae_descricao", "natureza_juridica", "porte", "capital_social", "simples", "mei", "endereco", "bairro",
    "cep", "municipio", "uf", "telefone", "email",
]


def fold(text: str) -> str:
    return "".join(c for c in unicodedata.normalize("NFD", text) if unicodedata.category(c) != "Mn").lower()


def day(year: int, month: int, d: int) -> int:
    def leaps(n: int) -> int:
        return n // 4 - n // 100 + n // 400

    def month_days(y: int, m: int) -> int:
        if m == 2:
            return 29 if (y % 4 == 0 and (y % 100 != 0 or y % 400 == 0)) else 28
        return 30 if m in (4, 6, 9, 11) else 31

    return (year - 1900) * 365 + leaps(year - 1) - leaps(1899) + sum(month_days(year, m) for m in range(1, month)) + d


def year_range(y: int) -> dict:
    return {"from": day(y, 1, 1), "to": day(y, 12, 31)}


def load():
    municipalities = {}
    with CATALOG.open(encoding="utf-8") as f:
        for line in f.read().splitlines()[1:]:
            cols = line.split("\t")
            municipalities[(cols[4], cols[3])] = int(cols[0])
    sectors = {}
    for i, s in enumerate(tomllib.load(SECTORS.open("rb"))["sector"]):
        sectors[s["id"]] = {"id": i, "cnaes": [{"subclass": int(c)} for c in sorted(set(s["cnae_core"]))]}
    return municipalities, sectors


MUNIS, SECTOR_IDS = load()

# (sector key, plural surface, article, setor phrase)
SECTORS = [
    ("bakery", "padarias", "as", "padarias"),
    ("pharmacy", "farmácias", "as", "farmácias"),
    ("restaurant", "restaurantes", "os", "restaurantes"),
    ("it_services", "empresas de TI", "as", "TI"),
    ("pet_shop", "pet shops", "os", "pet shops"),
    ("gym", "academias", "as", "academias"),
]
PLACES = [("Cuiabá", "MT"), ("Sinop", "MT"), ("Rondonópolis", "MT"), ("Sorriso", "MT"), ("Várzea Grande", "MT"), ("Goiânia", "GO"), ("Campo Grande", "MS"), ("Londrina", "PR")]


def ir(op, place, sector_key, status, extra=None, period=None):
    scope = {"place": place, "activity": {"sectors": {"ids": [SECTOR_IDS[sector_key]["id"]], "cnaes": SECTOR_IDS[sector_key]["cnaes"], "scope": "main"}}, "status": status}
    scope.update(extra or {})
    return {
        "op": op,
        "unit": "establishments",
        "scope": scope,
        "time": {"competence": CURRENT_COMP, "period": period, "anchor": ANCHOR_DAY},
        "output": {"page": 1},
        "versions": VERSIONS,
    }


LIST = {"kind": "list", "order": "cnpj", "limit": 20}


def muni(name, uf):
    return {"municipalities": [MUNIS[(name, uf)]]}


def uf(code):
    return {"ufs": [UFS.index(code)]}


def masc(article: str, feminine: str, masculine: str) -> str:
    return masculine if article == "os" else feminine


# Data parts: (kind, builder) -> (article, text after the article, gold IR)
def data_parts():
    for (key, plural, art, setor), (city, code), year in itertools.product(SECTORS, PLACES, (2024, 2025)):
        place = muni(city, code)
        if year == 2025:
            yield "active", art, f"{plural} {masc(art, 'ativas', 'ativos')} de {city}/{code}", ir(LIST, place, key, ["active"])
        yield "closed_year", art, f"{plural} que fecharam em {year} em {city}/{code}", ir(LIST, place, key, ["closed"], {"status_date": year_range(year)})
        yield "opened_year", art, f"{plural} {masc(art, 'abertas', 'abertos')} em {year} em {city}/{code}", ir(LIST, place, key, ALL_STATUS, {"opened": year_range(year)})
        yield "closed_year_adj", art, f"{plural} {masc(art, 'baixadas', 'baixados')} em {year} em {city}/{code}", ir(LIST, place, key, ["closed"], {"status_date": year_range(year)})
    for (key, plural, art, setor), code in itertools.product(SECTORS, ("MT", "GO", "MS")):
        yield "closed_all_uf", "todas as", f"empresas que tiveram baixa em {UF_NAMES[code]} no setor de {setor}", ir(LIST, uf(code), key, ["closed"])


FORMATS = {
    "xlsx": ["um Excel", "uma planilha", "um exel", "uma planilha Excel", "um arquivo xlsx"],
    "csv": ["um CSV", "um arquivo CSV"],
    "txt": ["um TXT", "um arquivo de texto"],
}
SUFFIX = {"xlsx": ["em Excel", "em planilha"], "csv": ["em CSV"], "txt": ["em TXT", "em txt"]}
OPENERS = ["Me mande", "Me envie", "Gere", "Quero", "Preciso de", "Por favor, me mande", "Pode me mandar"]
# (clause after "com"/"contendo", gold fields)
WITH_FIELDS = [
    ("nome, CNPJ e telefone", ["razao_social", "cnpj", "telefone"]),
    ("o CNPJ, a razão social e a data de abertura", ["cnpj", "razao_social", "data_inicio"]),
    ("CNPJ, nome fantasia, endereço e CEP", ["cnpj", "nome_fantasia", "endereco", "cep"]),
    ("todos os dados", ALL_FIELDS),
    ("o CNPJ e o e-mail", ["cnpj", "email"]),
    ("razão social, situação cadastral e data da baixa", ["razao_social", "situacao", "data_situacao"]),
]
# (clause starting with apenas/somente, gold fields, bare "número")
ONLY_FIELDS = [
    ("apenas o nome e o número", ["razao_social", "cnpj"], True),
    ("somente o CNPJ e a razão social", ["cnpj", "razao_social"], False),
    ("só o nome e o CNPJ", ["razao_social", "cnpj"], False),
    ("apenas o CNPJ, o porte e o capital social", ["cnpj", "porte", "capital_social"], False),
]
GENITIVE = {"as": "das", "os": "dos", "todas as": "de todas as"}


def export(fmt, fields=None):
    return {"format": fmt, "fields": fields or [], "rows": None}


def items():
    out = []
    parts = list(data_parts())
    cycle = itertools.count()

    def pick(seq):
        return seq[next(cycle) % len(seq)]

    for kind, art, text, gold in parts:
        fmt = pick(["xlsx", "csv", "txt", "xlsx"])
        opener = pick(OPENERS)
        # C: "<opener> <format> com <data>"
        out.append((f"{opener} {pick(FORMATS[fmt])} com {art} {text}", gold, export(fmt), "execute", f"format_prefix/{kind}"))
        # D: "Liste <data> <suffix>"
        out.append((f"Liste {art} {text} {pick(SUFFIX[fmt])}", gold, export(fmt), "execute", f"format_suffix/{kind}"))
        # A: "<opener> <format> com <fields> <genitive> <data>"
        clause, fields = pick(WITH_FIELDS)
        out.append((f"{opener} {pick(FORMATS[fmt])} com {clause} {GENITIVE[art]} {text}", gold, export(fmt, fields), "execute", f"with_fields/{kind}"))
        # B: "<opener> <only fields> em <format> com <data>" (the owner's example shape)
        clause, fields, bare = pick(ONLY_FIELDS)
        decision = "execute_with_note" if bare else "execute"
        out.append((f"{opener} {clause} em {pick(FORMATS[fmt])} com {art} {text}", gold, export(fmt, fields), decision, f"only_fields/{kind}"))
    # Exports of analytical answers keep their operation.
    for (key, plural, art, setor), code in itertools.product(SECTORS, ("MT", "GO")):
        gold = ir({"kind": "rank", "dim": "municipality", "metric": "stock", "dir": "desc", "k": 10, "min_base": 0}, uf(code), key, ["active"])
        out.append((f"Exporte em Excel o ranking das 10 cidades de {code} com mais {plural} {masc(art, 'ativas', 'ativos')}", gold, export("xlsx"), "execute", "analytical/rank"))
    return out


# Regional requests: (opener, region). Gold does not depend on the opener.
REGIONAL_OPENERS = [
    ("Bah, tchê, me passa", "sul"),
    ("Barbaridade, guri, me alcança", "sul"),
    ("Oxe, gera", "nordeste"),
    ("Oxente, me manda", "nordeste"),
    ("Ô meu rei, me envia", "nordeste"),
    ("Uai sô, cê consegue me mandar", "mineiro"),
    ("Uai, me arruma", "mineiro"),
    ("Vôte, me manda aí", "centro_oeste"),
    ("Tchá por Deus, gera", "centro_oeste"),
    ("Égua, maninho, me manda", "norte"),
    ("Mano, joga num", "sudeste"),
    ("Mermão, manda aí", "sudeste"),
    ("Fala, parceiro, bota numa", "sudeste"),
    ("Opa, dá pra me mandar", "geral"),
    ("Tem como me passar", "geral"),
]
# (surface, format or None for a file noun that defaults to Excel with a note)
REGIONAL_FILES = [
    ("uma planilha", "xlsx"), ("um Excel", "xlsx"), ("um ecxel", "xlsx"), ("uma planilhinha", "xlsx"), ("um CSV", "csv"),
    ("um TXT", "txt"), ("uma tabela", None), ("um relatório", None), ("um arquivo", None),
]
REGIONAL_TAILS = ["", ", visse?", ", por favor", " aí", ", valeu"]


def regional():
    out = []
    parts = [p for p in data_parts() if p[0] in ("active", "closed_year")]
    combos = itertools.product(REGIONAL_OPENERS, REGIONAL_FILES)
    for n, ((opener, region), (surface, fmt)) in enumerate(combos):
        kind, art, text, gold = parts[(n * 7) % len(parts)]
        tail = REGIONAL_TAILS[n % len(REGIONAL_TAILS)]
        # "joga num"/"bota numa" already carry the article of the file.
        noun = surface.split(" ", 1)[1] if opener.endswith((" num", " numa")) else surface
        if opener.endswith(" num") and surface.startswith("uma "):
            continue
        if opener.endswith(" numa") and surface.startswith("um "):
            continue
        decision = "execute" if fmt else "execute_with_note"
        out.append((f"{opener} {noun} {GENITIVE[art]} {text}{tail}", gold, export(fmt or "xlsx"), decision, f"regional/{region}"))
    return out


def dialogues():
    sessions = []
    for i, ((key, plural, art, setor), (city, code)) in enumerate(itertools.product(SECTORS, PLACES[:5])):
        place = muni(city, code)
        closures = ir({"kind": "count", "metric": {"flow": "closures"}}, place, key, ["closed"], period={"days": year_range(2025)})
        listed = ir(LIST, place, key, ["closed"], {"status_date": year_range(2025)})
        sid = f"skills-export-01-d{i:03d}"
        sessions.append([
            (sid, f"Quantas {plural} tiveram baixa em {city}/{code} em 2025?", closures, None, "execute", "new_query"),
            (sid, "Agora me mande isso em Excel", listed, export("xlsx"), "execute", "export"),
            (sid, "Só o nome e o CNPJ", listed, export("xlsx", ["razao_social", "cnpj"]), "execute", "export"),
            (sid, "Agora em CSV", listed, export("csv", ["razao_social", "cnpj"]), "execute", "export"),
        ])
        stock = ir({"kind": "count", "metric": "stock"}, place, key, ["active"])
        heads = json.loads(json.dumps(stock))
        heads["scope"]["head_office"] = "yes"
        listed_heads = json.loads(json.dumps(heads))
        listed_heads["op"] = LIST
        sid = f"skills-export-01-e{i:03d}"
        sessions.append([
            (sid, f"Quantas {plural} {masc(art, 'ativas', 'ativos')} existem em {city}/{code}?", stock, None, "execute", "new_query"),
            (sid, "E só as matrizes?", heads, None, "execute", None),
            (sid, "Agora liste esses estabelecimentos", listed_heads, None, "execute", None),
            (sid, "Me mande isso em TXT", listed_heads, export("txt"), "execute", "export"),
        ])
    return sessions


STOPS = [
    ("Me mande um Excel", "clarify"),
    ("Gere uma planilha", "clarify"),
    ("Me mande um Excel com o faturamento das padarias de Cuiabá/MT", "reject"),
    ("Gere uma planilha com os CPFs dos sócios das farmácias de Sinop/MT", "reject"),
    ("Me mande um CSV com o lucro dos restaurantes de Sorriso/MT", "reject"),
    ("Quero um TXT com os donos das academias de Cuiabá/MT", "reject"),
    ("Me manda um PDF das padarias de Sinop/MT", "clarify"),
    ("Bah, gera um Word com as farmácias de Cuiabá/MT", "clarify"),
    ("Oxe, me manda em JSON os restaurantes de Sorriso/MT", "clarify"),
]


def main():
    rows = []
    seen = set()

    def add(item_id, text, gold_ir, gold_export, decision, family, act="new_query", session=None):
        # One item per surface; a repeated surface is dropped, never relabelled.
        key = (session or "", fold(text))
        if key in seen:
            return
        seen.add(key)
        gold = {"decision": decision, "ir": gold_ir, "export": gold_export}
        if act:
            gold["act"] = act
        row = {"id": item_id, "text": text, "gold": gold, "meta": {"category": family.split("/")[0], "family": family, "human_validated": False}}
        if session:
            row["context"] = {"session_id": session}
        rows.append(row)

    for n, (text, gold_ir, gold_export, decision, family) in enumerate(items()):
        add(f"skills-export-01-{n:05d}", text, gold_ir, gold_export, decision, family)
    for n, (text, gold_ir, gold_export, decision, family) in enumerate(regional()):
        add(f"skills-export-01-r{n:04d}", text, gold_ir, gold_export, decision, family)
    for session in dialogues():
        for turn, (sid, text, gold_ir, gold_export, decision, act) in enumerate(session):
            add(f"{sid}#{turn}", text, gold_ir, gold_export, decision, "dialogue/" + ("export" if gold_export else "data"), act, sid)
    for n, (text, decision) in enumerate(STOPS):
        add(f"skills-export-01-s{n:03d}", text, None, None, decision, f"stop/{decision}")
    lines = "".join(json.dumps(row, ensure_ascii=False) + "\n" for row in rows)
    path = EVAL_DIR / "questions.jsonl"
    path.write_text(lines, encoding="utf-8")
    families = {}
    for row in rows:
        families[row["meta"]["family"]] = families.get(row["meta"]["family"], 0) + 1
    manifest = {
        "set": "skills-export-01",
        "items": len(rows),
        "sha256": hashlib.sha256(lines.encode()).hexdigest(),
        "families": dict(sorted(families.items())),
        "anchor": "2026-10-11",
        "generator": "author_skills_export.py",
    }
    (EVAL_DIR / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(manifest, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
