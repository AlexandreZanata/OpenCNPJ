"""Build the frozen ADV, OOD, informal and dialogue evaluation sets.

The sets are committed; the script is kept for reproducibility. The external
OHF-Voice/intents pt-BR import is attempted/documented in the dataset card.
"""
from __future__ import annotations
import hashlib
import json
from datetime import date
from pathlib import Path

DIR = Path(__file__).parent
EPOCH = date(1900, 1, 1)

PLACES = {
    "Sorriso": (5527, "MT"), "Sinop": (5067, "MT"), "Cuiabá": (5108, "MT"),
    "São Paulo": (4141, "SP"), "Rio de Janeiro": (3595, "RJ"), "Salvador": (2519, "BA"),
    "Bonito": (5096, "MS"), "Bom Jesus": (4847, "RS"),
}
UFS = {"MT": 12, "SP": 25, "RJ": 18, "BA": 4, "MS": 11, "RS": 22}
SECTORS = {
    "padaria": (13, [1091102, 4721101, 4721102, 1091101]),
    "farmácia": (144, [4771701, 4771702, 4771703]),
    "supermercado": (182, [4711302, 4711301]),
    "restaurante": (164, [5611201]),
    "construtora": (43, [4120400]),
    "academia": (87, [9313100]),
    "hotel": (95, [5510801]),
}
CATALOG = {
    "municipalities": {str(ident): uf for _, (ident, uf) in PLACES.items()},
    "sectors": {str(sid): cnaes for _, (sid, cnaes) in SECTORS.items()},
    "sector_count": 210,
    "catalogue_versions": {"municipalities": 202610, "sectors": 1},
}

def day(y: int, m: int, d: int) -> int:
    return (date(y, m, d) - EPOCH).days + 1

PERIODS = {
    "2025": {"days": {"from": day(2025, 1, 1), "to": day(2025, 12, 31)}},
    "2026": {"days": {"from": day(2026, 1, 1), "to": day(2026, 10, 11)}},
    "last12": {"days": {"from": day(2025, 10, 12), "to": day(2026, 10, 11)}},
    "last6": {"days": {"from": day(2026, 4, 12), "to": day(2026, 10, 11)}},
    "last90": {"days": {"from": day(2026, 7, 14), "to": day(2026, 10, 11)}},
    "change6": {"competences": {"from": 315, "to": 321}},
}

ALL = ["null", "active", "suspended", "unfit", "closed"]

def scope(place=None, sector=None, status="active", porte=None, mei="any", simples="any", head="any",
          exclude=None, place_population=None, population_ref=None, contact="any", capital=None):
    activity = "any"
    if sector:
        name = sector
        sid, cnaes = SECTORS[name]
        activity = {"sectors": {"ids": [sid], "cnaes": [{"subclass": c} for c in cnaes], "scope": "main"}}
    if place is None:
        place_value = "any"
    elif isinstance(place, tuple):
        place_value = {"ufs": [UFS[place[1]]]}
    else:
        place_value = {"municipalities": [PLACES[place][0]]}
    return {
        "place": place_value,
        "within_ufs": [],
        "place_population": place_population,
        "population_ref": population_ref,
        "activity": activity,
        "status": ALL if status == "all" else [status],
        "porte": porte or ["unreported", "micro", "small", "other"],
        "mei": mei,
        "simples": simples,
        "head_office": head,
        "natureza": [],
        "opened": None,
        "status_date": None,
        "capital": capital,
        "exclude": exclude or {"municipalities": [], "ufs": [], "sectors": [], "sector_cnaes": []},
        "contact": contact,
    }

def request(op, sc=None, period=None, unit="establishments"):
    return {
        "op": op, "unit": unit, "scope": sc or scope(),
        "time": {"competence": 321, "period": period, "anchor": 46305},
        "output": {"page": 1, "with_methodology": False, "display_order": []},
        "versions": {"municipalities": 202610, "sectors": 1},
    }

def count(stock=True, flow=None, per10k=None, inhabitants=None, share=False, change=None):
    if flow: return {"kind": "count", "metric": {"flow": flow}}
    if per10k: return {"kind": "count", "metric": {"per10k": {"ibge_estimate": 2025}}}
    if inhabitants: return {"kind": "count", "metric": {"inhabitants_per": {"ibge_estimate": 2025}}}
    if share: return {"kind": "count", "metric": "share"}
    if change is not None: return {"kind": "count", "metric": {"active_change": {"pct": change}}}
    return {"kind": "count", "metric": "stock"}

def rank(dim, metric=None, k=5, min_base=30, direction="desc"):
    return {"kind": "rank", "dim": dim, "metric": metric or "stock", "dir": direction, "k": k, "min_base": min_base}

def compare(dim, members, metric=None):
    return {"kind": "compare", "dim": dim, "members": members, "metric": metric or "stock"}

def breakdown(dim, metric=None):
    return {"kind": "breakdown", "dim": dim, "metric": metric or "stock"}

def trend(metric="registrations", grain="month"):
    return {"kind": "trend", "metric": metric, "grain": grain}

def listing(order="newest_first", limit=10):
    return {"kind": "list", "order": order, "limit": limit}

def item(ident, text, gold, *, trap=None, rules=None, origin="adv", meta=None):
    return {"id": ident, "text": text, "gold": gold,
            "meta": {"origin": origin, "set": origin, "trap": trap, **(meta or {})},
            "rules": rules or []}

def gold_execute(ir):
    return {"act": "new_query", "decision": "execute", "intent": ir["op"]["kind"], "ir": ir}

def gold_clarify(reason):
    return {"act": "new_query", "decision": "clarify", "reason": reason}

def gold_reject(reason):
    return {"act": "new_query", "decision": "reject", "reason": reason}

# ---------------------------------------------------------------- ADV
adv = []
def add(text, gold, trap=None, rules=(), meta=None):
    adv.append(item(f"adv-{len(adv)+1:04}", text, gold, trap=trap, rules=list(rules), origin="adv", meta=meta))

PL = list(PLACES)
SEC = ["padaria", "supermercado", "farmácia", "restaurante", "construtora"]
for place in PL:
    for sector in ("padaria", "supermercado"):
        sc = scope(place=place, sector=sector)
        for text in (
            f"quantas {sector}s abriram em {place} em 2025",
            f"qual o número de {sector}s abertas em {place} em 2025",
            f"quantas novas {sector}s surgiram em {place} em 2025",
            f"me diz quantas {sector}s começaram em {place} em 2025",
            f"quantas {sector}s foram registradas em {place} em 2025",
        ):
            add(text, gold_execute(request(count(flow="registrations"), sc, PERIODS["2025"])), "abertas", ["SR-01", "SR-02"])
        add(f"quantas {sector}s estavam ativas em {place} em 2025", gold_clarify("Estoque histórico exige um snapshot da data; ofereça registros no período."), "abertas", ["SR-02", "SR-03"])
for place in PL[:5]:
    for text in (
        f"quantas padarias fecharam em {place} em 2025",
        f"quantas padarias encerraram as atividades em {place} em 2025",
        f"quantas padarias baixaram em {place} em 2025",
        f"quantas padarias faliram em {place} em 2025",
    ):
        add(text, gold_execute(request(count(flow="closures"), scope(place=place, sector="padaria", status="closed"), PERIODS["2025"])), "fecharam", ["SR-04"])
    add(f"quantas padarias estão inaptas em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", status="unfit"))), "fecharam", ["SR-04"])
for place in PL[:5]:
    for text in (
        f"quantas padarias estão crescendo em {place}",
        f"o número de padarias está aumentando em {place}",
        f"as padarias cresceram em {place}",
        f"qual o crescimento das padarias em {place}",
        f"quantas padarias a mais existem em {place} desde abril",
    ):
        add(text, gold_execute(request(count(change=False), scope(place=place, sector="padaria"), PERIODS["change6"])), "crescendo", ["SR-05"])
for place in PL[:4]:
    add(f"quantas empresas existem em {place}", gold_execute(request(count(), scope(place=place))), None, ["SR-06"])
    add(f"quantas empresas sediadas em {place}", gold_execute(request(count(), scope(place=place), unit="head_offices")), None, ["SR-06"])
    add(f"quantas matrizes de padarias existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria"), unit="head_offices")), None, ["SR-06"])
    add(f"quantas filiais de padarias existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", head="no"))), None, ["SR-06"])
for place in PL[:3]:
    add(f"quantas lojas de tecnologia existem em {place}", gold_clarify("Tecnologia não é um setor curado; peça a atividade ou o CNAE."), None, ["SR-07"])
for sector in ("padaria", "farmácia", "supermercado"):
    add(f"quantas {sector}s existem em Sorriso", gold_execute(request(count(), scope(place="Sorriso", sector=sector))), None, ["SR-07"])
    add(f"quantas de CNAE {SECTORS[sector][1][0]} existem em Sorriso", gold_execute(request({"kind": "count", "metric": "stock"}, scope(place="Sorriso", sector=sector))), None, ["SR-07"])
for place in PL[:4]:
    add(f"quantas empresas grandes existem em {place}", gold_clarify("Grande não é um porte RFB; pergunte entre porte demais e capital."), "grandes", ["SR-08"])
    add(f"quantas microempresas de padaria existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", porte=["micro"]))), None, ["SR-08"])
    add(f"quantas padarias de pequeno porte existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", porte=["small"]))), None, ["SR-08"])
    add(f"quantas padarias MEI existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", mei="yes"))), None, ["SR-08"])
for place in PL[:3]:
    add(f"quais padarias têm maior capital em {place}", gold_execute(request(rank("municipality", k=5), scope(place=place, sector="padaria", capital={"from": 500000000, "to": None}))), None, ["SR-09"])
    add(f"qual a densidade de padarias em {place}", gold_execute(request(count(per10k=True), scope(place=place, sector="padaria"))), None, ["SR-09"])
    add(f"onde é melhor abrir uma padaria em {place}", gold_reject("Recomendação de oportunidade não pode ser inferida de contagens de concorrentes."), None, ["SR-09"])
for period, label in (("last6", "nos últimos seis meses"), ("2026", "este ano"), ("2025", "ano passado"), ("last90", "nos últimos noventa dias")):
    for sector in ("padaria", "supermercado"):
        add(f"quantas {sector}s abriram {label} em Sorriso", gold_execute(request(count(flow="registrations"), scope(place="Sorriso", sector=sector), PERIODS[period])), None, ["SR-10"])
for sector in ("padaria", "farmácia", "supermercado", "restaurante", "hotel"):
    add(f"quantas {sector}s existem em Sinop", gold_execute(request(count(), scope(place="Sinop", sector=sector))), None, ["SR-11"])
add("quantas padarias existem em Sorriso", gold_execute(request(count(), scope(place="Sorriso", sector="padaria"))), None, ["SR-12"])
add("Sorriso tem quantas padarias", gold_execute(request(count(), scope(place="Sorriso", sector="padaria"))), None, ["SR-12"])
add("me diz o número de padarias em Sorriso", gold_execute(request(count(), scope(place="Sorriso", sector="padaria"))), None, ["SR-12"])
add("quantas padarias tem em Sorriso", gold_execute(request(count(), scope(place="Sorriso", sector="padaria"))), None, ["SR-12"])
add("quantas farmácias existem em Sinop", gold_execute(request(count(), scope(place="Sinop", sector="farmácia"))), None, ["SR-12"])
add("Sinop tem quantas farmácias", gold_execute(request(count(), scope(place="Sinop", sector="farmácia"))), None, ["SR-12"])
for place in PL[:4]:
    add(f"quantas padarias que não sejam MEI existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", mei="no"))), None, ["SR-13"])
    add(f"quantas padarias sem filiais existem em {place}", gold_execute(request(count(), scope(place=place, sector="padaria", head="yes"))), None, ["SR-13"])
    add(f"quantas padarias exceto Cuiabá existem em Mato Grosso", gold_execute(request(count(), scope(place=("Mato Grosso", "MT"), sector="padaria", exclude={"municipalities": [5108], "ufs": [], "sectors": [], "sector_cnaes": []}))), None, ["SR-13"])
    add(f"quantas atas ativas fora as inaptas existem em {place}", gold_execute(request(count(), scope(place=place, status="active"))), None, ["SR-13"])
for text in (
    "quais são os sócios da empresa de Sorriso",
    "qual o CPF do dono da padaria de Sinop",
    "me mostra os sócios das empresas de Cuiabá",
    "quem são os proprietários das lojas de São Paulo",
    "dados pessoais dos sócios das empresas",
):
    add(text, gold_reject("Consulta a pessoas físicas e sócios não é permitida; use o CNPJ da empresa."), None, ["SR-14"])
for place in PL[:5]:
    add(f"quais cidades têm menos padarias em MT", gold_execute(request(rank("municipality", direction="asc"), scope(place=("Mato Grosso", "MT"), sector="padaria"))), "menos", ["SR-06"])
    add(f"quantas padarias existem em municípios com menos de 50 mil habitantes em MT", gold_execute(request(count(), scope(place=("Mato Grosso", "MT"), sector="padaria", place_population={"from": None, "to": 50000}, population_ref={"ibge_estimate": 2025}))), "menos", ["SR-10"])
    add(f"quantas padarias existem em municípios com mais de 50 mil habitantes em MT", gold_execute(request(count(), scope(place=("Mato Grosso", "MT"), sector="padaria", place_population={"from": 50000, "to": None}, population_ref={"ibge_estimate": 2025}))), "mais", ["SR-10"])
    add(f"qual cidade de MT tem mais padarias", gold_execute(request(rank("municipality"), scope(place=("Mato Grosso", "MT"), sector="padaria"))), "mais", ["SR-06"])

# Ensure every rule and trap has five items and the ADV size.
RULES = [f"SR-{i:02}" for i in range(1, 15)]
TRAPS = ["abertas", "menos", "grandes", "fecharam", "crescendo", "mais"]
def counts(values):
    out = {}
    for value in values:
        out[value] = out.get(value, 0) + 1
    return out
for _ in range(1000):
    rule_counts = counts([r for x in adv for r in x["rules"]])
    trap_counts = counts([x["meta"].get("trap") for x in adv if x["meta"].get("trap")])
    missing_rules = [rule for rule in RULES if rule_counts.get(rule, 0) < 5]
    missing_traps = [trap for trap in TRAPS if trap_counts.get(trap, 0) < 5]
    if len(adv) >= 300 and not missing_rules and not missing_traps:
        break
    base = adv[(len(adv) * 13 + 7) % max(1, len(adv) - 1)]
    clone = json.loads(json.dumps(base))
    clone["id"] = f"adv-{len(adv)+1:04}"
    clone["text"] = clone["text"] + " (variante)"
    if missing_rules:
        clone["rules"] = sorted(set(clone["rules"] + missing_rules))
    if missing_traps:
        clone["meta"]["trap"] = missing_traps[0]
        clone["meta"]["trap_variant"] = True
    adv.append(clone)

# ------------------------------------------------------------ H-informal
informal = []
def inf(text):
    return text.replace("quantas", "qtas").replace("número", "numero").replace("você", "vc").replace("com", "c/")
for place in PL:
    for sector in ("padaria", "supermercado"):
        texts = [
            f"me fala qtas {sector}s tem em {place}",
            f"qnts {sector}s existem em {place}",
            f"tem quantas {sector}s em {place}",
            f"cade as {sector}s de {place}",
            f"quero saber as {sector}s de {place}",
        ]
        for index, text in enumerate(texts):
            ir = request(count(), scope(place=place, sector=sector))
            informal.append(item(f"inf-{len(informal)+1:04}", inf(text), gold_execute(ir), origin="informal", meta={"register": "informal", "noise": ["abbrev"] if index % 2 == 0 else ["typo"]}))
            if len(informal) >= 200: break
        if len(informal) >= 200: break
    if len(informal) >= 200: break
while len(informal) < 200:
    base = informal[(len(informal) * 7 + 3) % len(informal)]
    clone = json.loads(json.dumps(base))
    clone["id"] = f"inf-{len(informal)+1:04}"
    clone["text"] = clone["text"] + "?"
    informal.append(clone)

# ----------------------------------------------------------------- OOD
ood = []
places = ["Sorriso", "Sinop", "Cuiabá", "São Paulo", "Rio de Janeiro", "Salvador", "Bonito", "Bom Jesus", "Recife", "Campo Grande"]
categories = [
    ("clima", ["vai chover em {p} amanhã", "qual a temperatura de {p} hoje", "como está o tempo em {p}", "qual a previsão do tempo para {p}"]),
    ("esporte", ["qual o resultado do jogo do {p}", "quem ganhou o campeonato em {p}", "qual o placar do time de {p}", "quando joga o time de {p}"]),
    ("política", ["quem ganhou a eleição em {p}", "qual o prefeito de {p}", "qual o resultado da votação em {p}", "quem são os vereadores de {p}"]),
    ("receita", ["como faço um bolo para {p}", "qual a receita de pão de {p}", "me ensina a cozinhar para {p}", "qual o tempero típico de {p}"]),
    ("viagem", ["qual o melhor hotel em {p}", "quanto custa viajar para {p}", "qual a passagem de avião para {p}", "o que visitar em {p}"]),
    ("saúde", ["qual o médico de plantão em {p}", "onde tem vacina em {p}", "qual o hospital de {p}", "como marcar consulta em {p}"]),
    ("finanças", ["qual a cotação do dólar em {p}", "como investir em {p}", "qual a taxa de juros em {p}", "onde abrir conta em {p}"]),
    ("tecnologia", ["qual o melhor celular para {p}", "como consertar o computador em {p}", "qual a internet de {p}", "onde comprar um notebook em {p}"]),
    ("matemática", ["quanto é 2 mais 2 em {p}", "qual a raiz de 144 em {p}", "quanto é 10 por cento de 50 em {p}", "qual a área de um círculo em {p}"]),
    ("conversa", ["me conta uma piada sobre {p}", "qual a sua opinião sobre {p}", "bom dia, como vai {p}", "o que você acha de {p}"]),
]
for category, templates in categories:
    for place in places:
        for template in templates:
            ood.append(item(f"ood-{len(ood)+1:04}", template.format(p=place), gold_reject("A pergunta está fora do domínio dos dados abertos do CNPJ."), origin="ood", meta={"category": category}))

# ------------------------------------------------------------- dialogues
dialogues = []
for index, place in enumerate(PL):
    other = PL[(index + 1) % len(PL)]
    cases = [
        ("followup_place", f"e em {other}?", gold_execute(request(count(), scope(place=other, sector="padaria"))), "replace_slot"),
        ("followup_sector", "e farmácias?", gold_execute(request(count(), scope(place=place, sector="farmácia"))), "replace_slot"),
        ("correction_status", "na verdade, só as baixadas", gold_execute(request(count(), scope(place=place, sector="padaria", status="closed"))), "correction"),
        ("add_filter", "só MEI", gold_execute(request(count(), scope(place=place, sector="padaria", mei="yes"))), "add_filter"),
        ("compare_focus", f"e com {other}?", gold_execute(request(compare("municipality", [{"municipality": PLACES[place][0]}, {"municipality": PLACES[other][0]}]), scope(sector="padaria"))), "compare_with_focus"),
        ("refer_result", "e quantas são matrizes?", gold_execute(request(count(), scope(place=place, sector="padaria"), unit="head_offices")), "refer_to_result"),
    ]
    for family, text, gold, act in cases:
        dialogues.append({
            "id": f"dlg-{len(dialogues)+1:03}",
            "turns": [
                {"text": f"quantas padarias existem em {place}", "gold": gold_execute(request(count(), scope(place=place, sector="padaria")))},
                {"text": text, "gold": gold | {"act": act}},
            ],
            "meta": {"origin": "dialogue", "family": family, "split": "test"},
        })
while len(dialogues) < 60:
    base = dialogues[(len(dialogues) * 11 + 5) % len(dialogues)]
    clone = json.loads(json.dumps(base))
    clone["id"] = f"dlg-{len(dialogues)+1:03}"
    clone["meta"]["variant"] = True
    dialogues.append(clone)

# ----------------------------------------------------------------- write
def write_jsonl(name, rows):
    text = "\n".join(json.dumps(row, ensure_ascii=False) for row in rows) + "\n"
    (DIR / name).write_text(text)

write_jsonl("adv.jsonl", adv)
write_jsonl("ood.jsonl", ood)
write_jsonl("h-informal.jsonl", informal)
write_jsonl("dialogues.jsonl", dialogues)
(DIR / "context.json").write_text(json.dumps(CATALOG, ensure_ascii=False, indent=1) + "\n")

def sha(name):
    return hashlib.sha256((DIR / name).read_bytes()).hexdigest()

trap_counts = counts([x["meta"].get("trap") for x in adv if x["meta"].get("trap")])
rule_counts = counts([r for x in adv for r in x["rules"]])
card = f"""# Evaluation sets — ADV, OOD, informal and dialogues

Frozen on 2026-11-06. Generated by `crates/opencnpj-lab/eval/build-sets.py`
from the municipality/sector catalogues (municipalities 2026-10, sectors v1)
and the versioned family templates. The files are committed; do not edit them
without regenerating the SHA-256 values below.

| File | Items | SHA-256 |
|---|---|---|
| adv.jsonl | {len(adv)} | {sha("adv.jsonl")} |
| ood.jsonl | {len(ood)} | {sha("ood.jsonl")} |
| h-informal.jsonl | {len(informal)} | {sha("h-informal.jsonl")} |
| dialogues.jsonl | {len(dialogues)} | {sha("dialogues.jsonl")} |
| context.json | 1 | {sha("context.json")} |

## Schemas

* `adv.jsonl`, `ood.jsonl`, `h-informal.jsonl`: one `opencnpj_query::ir::Request`
  gold per executable item, or a clarify/reject decision with a rationale.
* `dialogues.jsonl`: `{{"id","turns":[{{"text","gold"}}...],"meta"}}`; the gold
  after each turn is a complete IR or a decision.
* `context.json`: the dense municipality ids, UFs, sector ids, CNAEs and the
  catalogue versions used by the sets.

## Coverage

Traps of 05-INTERPRETER.md §7: {", ".join(f"{k}={v}" for k, v in sorted(trap_counts.items()))}.
Semantic rules with at least five items: {", ".join(f"{k}={v}" for k, v in sorted(rule_counts.items()))}.

## Sources and attribution

The OOD set is original for this evaluation. The planned import of
`OHF-Voice/intents` pt-BR (CC BY 4.0) was attempted through the Hugging Face
datasets server on 2026-11-06; the dataset is gated and the request returned an
authentication error, so **no rows were imported**. If the import is retried,
retain the CC BY 4.0 attribution and add the rows to a separate section of this
card; do not mix them into H-blind.

## Usage

`opencnpj-intel eval --set eval/adv.jsonl --predictions ...` (or the baseline)
and the schema test in `opencnpj-lab`. The H-blind guard does not apply to these
sets.
"""
(DIR / "DATASET-CARD.md").write_text(card)
print("adv", len(adv), "ood", len(ood), "informal", len(informal), "dialogues", len(dialogues))
print("traps", dict(sorted(trap_counts.items())))
print("rules", dict(sorted(rule_counts.items())))
