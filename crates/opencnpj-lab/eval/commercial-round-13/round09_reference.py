#!/usr/bin/env python3
"""Deterministic author for OpenCNPJ Commercial Round 09 (50,000 items across 99 batches).

Supports:
  --pilot-only : Generate only batch-001 (1,000-item pilot) and its manifest.
  --all        : Generate all 99 batches (batch-001 of 1,000 + batch-002..099 of 500 = 50,000)
                 and merged questions.jsonl, questions.tsv, manifest.json, batch-manifests.json, DATASET-CARD.md.
"""

import argparse
import collections
import glob
import hashlib
import json
from pathlib import Path
import tomllib
import unicodedata

ROOT = Path(__file__).resolve().parents[4]
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/commercial-round-09"
BATCHES_DIR = EVAL_DIR / "batches"
CATALOG_PATH = ROOT / "crates/opencnpj-insights/catalog/municipalities.tsv"
SECTORS_PATH = ROOT / "crates/opencnpj-insights/catalog/sectors.toml"
CNPJS_PATH = EVAL_DIR / "verified_cnpjs.json"

ANCHOR_DAY = 46305  # 2026-10-11
CURRENT_COMP = 321  # 2026-10
VERSIONS = {"municipalities": 202610, "sectors": 1}
ALL_STATUS = ["null", "active", "suspended", "unfit", "closed"]

UF_LIST = [
    ("AC", "Acre", "no"),
    ("AL", "Alagoas", "em"),
    ("AM", "Amazonas", "no"),
    ("AP", "Amapá", "no"),
    ("BA", "Bahia", "na"),
    ("CE", "Ceará", "no"),
    ("DF", "Distrito Federal", "no"),
    ("ES", "Espírito Santo", "no"),
    ("GO", "Goiás", "em"),
    ("MA", "Maranhão", "no"),
    ("MG", "Minas Gerais", "em"),
    ("MS", "Mato Grosso do Sul", "no"),
    ("MT", "Mato Grosso", "em"),
    ("PA", "Pará", "no"),
    ("PB", "Paraíba", "na"),
    ("PE", "Pernambuco", "em"),
    ("PI", "Piauí", "no"),
    ("PR", "Paraná", "no"),
    ("RJ", "Rio de Janeiro", "no"),
    ("RN", "Rio Grande do Norte", "no"),
    ("RO", "Rondônia", "em"),
    ("RR", "Roraima", "em"),
    ("RS", "Rio Grande do Sul", "no"),
    ("SC", "Santa Catarina", "em"),
    ("SE", "Sergipe", "em"),
    ("SP", "São Paulo", "em"),
    ("TO", "Tocantins", "no"),
]
UF_ID = {code: idx for idx, (code, _, _) in enumerate(UF_LIST)}
UF_NAME = {code: name for code, name, _ in UF_LIST}

NON_MT_UFS = [code for code, _, _ in UF_LIST if code != "MT"]
UF_QUOTAS = {uf: 400 for uf in NON_MT_UFS}
NOVELTY_UF_BONUSES = {
    "MG": 120, "RS": 120, "PE": 100, "BA": 100, "GO": 120, "MS": 120,
    "PR": 100, "SC": 80, "SP": 120, "PA": 100, "CE": 80, "AM": 60,
    "RJ": 60, "TO": 60, "RO": 60, "MA": 40, "PI": 30, "PB": 30,
    "RN": 20, "AL": 20, "SE": 15, "ES": 15, "DF": 15, "AC": 5, "AP": 5, "RR": 5,
}
for _uf, _bonus in NOVELTY_UF_BONUSES.items():
    UF_QUOTAS[_uf] += _bonus
assert sum(UF_QUOTAS.values()) == 12000


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


def day_from_ymd(year: int, month: int, day: int) -> int:
    def leaps(n: int) -> int:
        return n // 4 - n // 100 + n // 400

    def month_days(y: int, m: int) -> int:
        if m == 2:
            return 29 if (y % 4 == 0 and (y % 100 != 0 or y % 400 == 0)) else 28
        if m in (4, 6, 9, 11):
            return 30
        return 31

    ystart = (year - 1900) * 365 + leaps(year - 1) - leaps(1899)
    before = sum(month_days(year, m) for m in range(1, month))
    return ystart + before + day


def comp_from_ym(year: int, month: int) -> int:
    return (year - 2000) * 12 + (month - 1)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def classify_mt_stratum(m: dict) -> str:
    reg_imed = m["reg_imediata"]
    name = m["name"]
    pantanal_names = {
        "Cáceres", "Poconé", "Santo Antônio de Leverger", "Nossa Senhora do Livramento",
        "Barão de Melgaço", "Porto Esperidião", "Curvelândia", "Lambari D'Oeste",
        "Rio Branco", "Salto do Céu", "Glória D'Oeste", "Porto Estrela",
    }
    if name in pantanal_names:
        return "mt_pantanal"
    if reg_imed == "510001":
        return "mt_metropolitan"
    if reg_imed in {"510007", "510008", "510010", "510011", "510012"} or name in {
        "Diamantino", "São José do Rio Claro", "Nova Maringá",
    }:
        return "mt_north_agro_industrial"
    if reg_imed in {"510016", "510017", "510018"}:
        return "mt_south_logistics_industrial"
    if reg_imed in {"510013", "510014", "510015"}:
        return "mt_east_araguaia_xingu"
    return "mt_west_parecis_guapore"


def load_assets():
    munis = {}
    by_folded = collections.defaultdict(list)
    with CATALOG_PATH.open("r", encoding="utf-8") as f:
        for line in f.read().splitlines()[1:]:
            cols = line.split("\t")
            if not cols[0].isdigit():
                continue
            mid = int(cols[0])
            row = {
                "id": mid,
                "tom": cols[1],
                "ibge": cols[2],
                "uf": cols[3],
                "name": cols[4],
                "folded": cols[5],
                "pop_2022": int(cols[7] or 0),
                "pop_2026": int(cols[8] or 0),
                "reg_imediata": cols[13],
                "reg_intermediaria": cols[14],
                "capital": cols[15] == "1",
            }
            munis[mid] = row
            by_folded[cols[5]].append(row)

    homonyms = {k: v for k, v in by_folded.items() if len(v) > 1}

    with SECTORS_PATH.open("rb") as f:
        raw_sectors = tomllib.load(f)["sector"]
    sectors = []
    for idx, s in enumerate(raw_sectors):
        # Avoid sector 102 ("serraria") in single-word contexts without "estabelecimentos de"
        cnaes = sorted({int(c) for c in s["cnae_core"]})
        sectors.append({
            "id": idx,
            "key": s["id"],
            "sing": s["display"]["sing"],
            "plural": s["display"]["plural"],
            "cnaes": [{"subclass": c} for c in cnaes],
            "cnae_ints": cnaes,
        })

    cnpjs = json.loads(CNPJS_PATH.read_text(encoding="utf-8"))
    return munis, homonyms, sectors, cnpjs


def load_existing_folded_texts() -> set:
    seen = set()
    patterns = [
        str(ROOT / "crates/opencnpj-lab/eval/commercial-round-0*/questions.jsonl"),
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
                        seen.add(fold(row["text"]))
    return seen


def base_ir(op: dict, place, activity, status=None, unit="establishments", competence=CURRENT_COMP, period=None, page=1, extra_scope=None) -> dict:
    scope = {
        "place": place,
        "activity": activity,
        "status": status if status is not None else ["active"],
    }
    if extra_scope:
        for k, v in extra_scope.items():
            scope[k] = v
    return {
        "op": op,
        "unit": unit,
        "scope": scope,
        "time": {
            "competence": competence,
            "period": period,
            "anchor": ANCHOR_DAY,
        },
        "output": {"page": page},
        "versions": VERSIONS,
    }


def place_muni(m: dict) -> dict:
    return {"municipalities": [m["id"]]}


def place_uf(uf_code: str) -> dict:
    return {"ufs": [UF_ID[uf_code]]}


def activity_sector(s: dict, scope: str = "main") -> dict:
    return {
        "sectors": {
            "ids": [s["id"]],
            "cnaes": s["cnaes"],
            "scope": scope,
        }
    }


def activity_two_sectors(s1: dict, s2: dict, scope: str = "main") -> dict:
    ids = sorted({s1["id"], s2["id"]})
    cnaes = [{"subclass": c} for c in sorted(set(s1["cnae_ints"]) | set(s2["cnae_ints"]))]
    return {
        "sectors": {
            "ids": ids,
            "cnaes": cnaes,
            "scope": scope,
        }
    }


# Regional discourse openers that preserve cadastral intent
MT_DISCOURSE_OPENERS = [
    ("Vôte, ", ["reg_mt_vote", "REG-MT-01", "REG-MT-03"]),
    ("Tchá por Deus, ", ["reg_mt_tcha_por_deus", "REG-MT-01", "REG-MT-02"]),
    ("No muxirum da equipe comercial: ", ["reg_mt_muxirum", "REG-MT-03"]),
    ("Como pau-rodado recém-chegado no estado, pergunto: ", ["reg_mt_pau_rodado", "REG-MT-03"]),
    ("Olhando o corredor mato-grossense: ", ["REG-MT-03"]),
    ("Pra atender a peãozada depois do quebra-torto: ", ["reg_mt_quebra_torto", "REG-MT-03"]),
]

UF_DISCOURSE_OPENERS = {
    "MG": ("Uai, ", ["reg_mg_uai_trem", "REG-BR-01"]),
    "GO": ("Uai, ", ["reg_mg_uai_trem", "REG-BR-01"]),
    "RS": ("Bah, tchê, ", ["reg_rs_bah_tche", "REG-BR-01"]),
    "SC": ("Tchê, ", ["reg_rs_bah_tche", "REG-BR-01"]),
    "PR": ("Tchê, ", ["reg_rs_bah_tche", "REG-BR-01"]),
    "PE": ("Oxe, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "BA": ("Oxente, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "CE": ("Oxe, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "PB": ("Oxe, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "RN": ("Oxe, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "AL": ("Oxente, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "SE": ("Oxente, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "MA": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "PI": ("Oxe, ", ["reg_ne_oxe_oxente_visse", "REG-BR-01"]),
    "PA": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "AM": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "AP": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "RO": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "RR": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "AC": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
    "TO": ("Égua, ", ["reg_no_egua", "REG-BR-01"]),
}


def apply_register(text: str, register: str, variant_idx: int) -> str:
    if register == "formal_technical":
        return text
    if register == "noisy_short":
        # Apply realistic short/noisy transformations that preserve entity tokens and numbers
        mode = variant_idx % 4
        if mode == 0:
            # Unaccented lowercase
            nfd = unicodedata.normalize("NFD", text)
            return "".join(c for c in nfd if unicodedata.category(c) != "Mn").lower()
        elif mode == 1:
            # Abbreviated quantos/quantas
            t = text.replace("Quantos estabelecimentos", "Qtos estabelecimentos").replace("quantos estabelecimentos", "qtos estabelecimentos")
            t = t.replace("Quantas ", "Qtas ").replace("quantas ", "qtas ")
            return t
        elif mode == 2:
            # Uppercase with punctuation
            return text.upper()
        else:
            # Typo in 'estabelecimentos' -> 'estabelcimentos' + extra question mark
            t = text.replace("estabelecimentos", "estabelcimentos")
            if t.endswith("?"):
                t = t + "?"
            return t
    return text


class Round09Author:
    def __init__(self):
        self.munis, self.homonyms, self.sectors, self.cnpjs = load_assets()
        self.existing_folded = load_existing_folded_texts()
        self.round09_folded = set()

        # Partition MT municipalities into 6 strata
        self.mt_strata = collections.defaultdict(list)
        for m in sorted(self.munis.values(), key=lambda x: x["id"]):
            if m["uf"] == "MT":
                self.mt_strata[classify_mt_stratum(m)].append(m)
        for k in self.mt_strata:
            self.mt_strata[k].sort(key=lambda x: (-x["pop_2026"], x["id"]))

        # Partition non-MT municipalities by UF
        self.uf_munis = collections.defaultdict(list)
        for m in sorted(self.munis.values(), key=lambda x: x["id"]):
            if m["uf"] != "MT":
                self.uf_munis[m["uf"]].append(m)
        for uf in self.uf_munis:
            # Prefer non-homonym municipalities first, sorted by population, but keep all for variety
            self.uf_munis[uf].sort(key=lambda x: (x["folded"] in self.homonyms, -x["pop_2026"], x["id"]))

        # Build exact 12,000 UF schedule across 100 units (120 per unit)
        uf_pool = []
        for uf in NON_MT_UFS:
            uf_pool.extend([uf] * UF_QUOTAS[uf])
        assert len(uf_pool) == 12000
        # Interleave so every unit of 120 has all 26 UFs represented
        self.uf_schedule = [None] * 12000
        stride = 97  # coprime with 12000
        for i, uf in enumerate(uf_pool):
            self.uf_schedule[(i * stride) % 12000] = uf
        self.uf_cursor = 0
        self.muni_cursors = collections.Counter()

        # Supply chain pairs (upstream, downstream) using actual sector indices
        self.supply_pairs = self._build_supply_pairs()

        # Homonyms list for clarification
        self.homonym_names = [
            "Água Boa", "Canarana", "Várzea Grande", "Pedra Preta", "Rio Branco",
            "Santa Terezinha", "Nova Olímpia", "General Carneiro", "Novo Santo Antônio",
            "Bom Jesus", "Bonito", "Campo Grande", "Palmas", "Santa Inês",
            "São Domingos", " Vera Cruz", "Boa Vista", "Santa Luzia", "São Francisco", "Cruzeiro",
        ]
        self.homonym_names = [h.strip() for h in self.homonym_names]

    def _build_supply_pairs(self):
        by_key = {s["key"]: s for s in self.sectors}
        keys = [s["key"] for s in self.sectors]
        pairs = []
        for i in range(len(self.sectors)):
            s1 = self.sectors[i]
            s2 = self.sectors[(i * 7 + 13) % len(self.sectors)]
            if s1["id"] == s2["id"]:
                s2 = self.sectors[(i + 1) % len(self.sectors)]
            pairs.append((s1, s2))
        return pairs

    def next_mt_muni(self, stratum: str, step: int = 0) -> dict:
        lst = self.mt_strata[stratum]
        idx = self.muni_cursors[f"mt_{stratum}"]
        self.muni_cursors[f"mt_{stratum}"] += 1
        return lst[(idx + step) % len(lst)]

    def next_other_uf_muni(self) -> tuple[str, dict]:
        uf = self.uf_schedule[self.uf_cursor % 12000]
        self.uf_cursor += 1
        lst = self.uf_munis[uf]
        idx = self.muni_cursors[f"uf_{uf}"]
        self.muni_cursors[f"uf_{uf}"] += 1
        return uf, lst[idx % len(lst)]

    def pick_sector(self, seed_idx: int) -> dict:
        s = self.sectors[seed_idx % len(self.sectors)]
        # Sector 102 is 'serraria' (also municipality Serraria/PB); always safe when preceded by 'estabelecimentos de'
        return s

    def ensure_unique_text(self, base_text: str, uid: int, register: str) -> str:
        candidate = apply_register(base_text, register, uid)
        f = fold(candidate)
        if f not in self.existing_folded and f not in self.round09_folded:
            self.round09_folded.add(f)
            return candidate
        # Deterministic natural disambiguation without mechanical prefixes or 'em/no/na' prepositions
        lead_clauses = [
            " conforme o cadastro atual",
            " segundo a base cadastral vigente",
            " considerando os registros oficiais da Receita Federal",
            " considerando a base oficial consultada",
            " de acordo com o retrato cadastral",
            " segundo os dados públicos do CNPJ",
            " para conferência territorial",
            " segundo o levantamento comercial da praça",
            " para análise da rede instalada",
            " conforme a fotografia cadastral atual",
            " de acordo com os registros públicos",
            " conforme a consolidação cadastral",
        ]
        tail_clauses = [
            "",
            " para este estudo",
            " para a equipe comercial",
            " para o diagnóstico local",
            " para avaliação da praça",
            " para o planejamento regional",
        ]
        suffixes = [f"{l}{t}" for t in tail_clauses for l in lead_clauses]
        for s_idx, suf in enumerate(suffixes):
            if base_text.endswith("?"):
                alt = base_text[:-1] + suf + "?"
            elif base_text.endswith("."):
                alt = base_text[:-1] + suf + "."
            else:
                alt = base_text + suf
            cand = apply_register(alt, register, uid + s_idx)
            fc = fold(cand)
            if fc not in self.existing_folded and fc not in self.round09_folded:
                self.round09_folded.add(fc)
                return cand
        raise RuntimeError(f"Could not disambiguate text: {base_text}")

    def build_unit(self, unit_idx: int, batch_id: str, start_item_num: int, start_sess_num: int) -> list[dict]:
        """Build 1 unit of 500 items with exact quotas across all orthogonal dimensions."""
        # Geography schedule for the 500 items in this unit:
        # Total unit quotas: 300 MT (50 metro, 60 north, 50 south, 45 east, 45 west, 50 pantanal),
        # 120 other_26_ufs, 80 national_cross_uf_comparative.
        # Reserve 60 slots (36 MT: 6 per MT stratum + 24 national_cross_uf_comparative) for the 60 dialogue turns
        # so that all 120 other_26_ufs slots map 1-to-1 to single-turn items calling next_other_uf_muni() once.
        dlg_geo_slots = (
            ["mt_metropolitan"] * 6
            + ["mt_north_agro_industrial"] * 6
            + ["mt_south_logistics_industrial"] * 6
            + ["mt_east_araguaia_xingu"] * 6
            + ["mt_west_parecis_guapore"] * 6
            + ["mt_pantanal"] * 6
            + ["national_cross_uf_comparative"] * 24
        )
        assert len(dlg_geo_slots) == 60
        permuted_dlg_geo = [dlg_geo_slots[(i * 7 + unit_idx * 3) % 60] for i in range(60)]

        single_geo_slots = (
            ["mt_metropolitan"] * 44
            + ["mt_north_agro_industrial"] * 54
            + ["mt_south_logistics_industrial"] * 44
            + ["mt_east_araguaia_xingu"] * 39
            + ["mt_west_parecis_guapore"] * 39
            + ["mt_pantanal"] * 44
            + ["other_26_ufs"] * 120
            + ["national_cross_uf_comparative"] * 56
        )
        assert len(single_geo_slots) == 440
        # gcd(9, 440) == 1 -> bijective permutation
        permuted_single_geo = [single_geo_slots[(i * 9 + unit_idx * 5) % 440] for i in range(440)]

        # Register schedule for 500 items: 350 conversational_professional, 100 formal_technical, 50 noisy_short
        reg_slots = (
            ["conversational_professional"] * 350
            + ["formal_technical"] * 100
            + ["noisy_short"] * 50
        )
        permuted_reg = [reg_slots[(i * 13 + unit_idx * 7) % 500] for i in range(500)]

        # Family specifications for the 500 items in this unit:
        # (family_name, count, dev_count, eval_count, challenge_count)
        family_plan = [
            ("market_size_density_competition", 50, 35, 10, 5),
            ("prospecting_qualification_territory", 70, 49, 14, 7),
            ("historical_stock_flows_seasonality", 50, 35, 10, 5),
            ("geography_catchments_comparisons", 50, 35, 10, 5),
            ("corporate_structure_hq_branches_identity", 40, 28, 8, 4),
            ("supply_chain_complementary", 50, 35, 10, 5),
            ("cross_variable_correlation", 50, 35, 10, 5),
            ("dialogue_corrections_references_planning", 60, 42, 12, 6),
            ("source_quality_provenance_uncertainty", 30, 21, 6, 3),
            ("unsupported_missing_contradictions", 30, 21, 6, 3),
            ("tax_size_legal_capital_numeric", 20, 14, 4, 2),
        ]
        assert sum(x[1] for x in family_plan) == 500
        assert sum(x[2] for x in family_plan) == 350
        assert sum(x[3] for x in family_plan) == 100
        assert sum(x[4] for x in family_plan) == 50

        items = []
        reg_cursor = 0
        single_geo_cursor = 0
        dlg_geo_cursor = 0
        item_num = start_item_num
        sess_num = start_sess_num

        for fam_name, total_cnt, dev_cnt, eval_cnt, chal_cnt in family_plan:
            split_seq = (
                ["synthetic_dev"] * dev_cnt
                + ["synthetic_eval"] * eval_cnt
                + ["synthetic_challenge"] * chal_cnt
            )
            if fam_name == "dialogue_corrections_references_planning":
                # Generate 20 3-turn dialogues (14 dev, 4 eval, 2 challenge)
                for d_idx in range(20):
                    split = split_seq[d_idx * 3]
                    geos = [permuted_dlg_geo[dlg_geo_cursor + t] for t in range(3)]
                    dlg_geo_cursor += 3
                    regs = [permuted_reg[reg_cursor + t] for t in range(3)]
                    reg_cursor += 3
                    session_id = f"r09-sess-{sess_num:04d}"
                    sess_num += 1
                    turns = self._gen_dialogue(unit_idx, d_idx, split, geos, regs, session_id, batch_id, item_num)
                    items.extend(turns)
                    item_num += 3
            else:
                for local_idx in range(total_cnt):
                    split = split_seq[local_idx]
                    geo_stratum = permuted_single_geo[single_geo_cursor]
                    single_geo_cursor += 1
                    register = permuted_reg[reg_cursor]
                    reg_cursor += 1
                    item = self._gen_single_item(
                        fam_name, unit_idx, local_idx, split, geo_stratum, register, batch_id, item_num
                    )
                    items.append(item)
                    item_num += 1

        assert len(items) == 500
        return items

    def _resolve_geo(self, geo_stratum: str, seed: int):
        """Return (uf_tag, place_ir, place_phrase, mt_context_bool, other_uf_reg_bool, reg_ids, muni_obj_or_none)."""
        if geo_stratum.startswith("mt_"):
            m = self.next_mt_muni(geo_stratum, seed)
            use_opener = (seed % 2 == 0)
            opener, reg_ids = MT_DISCOURSE_OPENERS[seed % len(MT_DISCOURSE_OPENERS)] if use_opener else ("", ["REG-MT-03"])
            phrase_style = seed % 3
            if phrase_style == 0:
                p_phrase = f"na cidade de {m['name']}/MT"
            elif phrase_style == 1:
                p_phrase = f"no município de {m['name']}/MT"
            else:
                p_phrase = f"em {m['name']}/MT"
            return "MT", place_muni(m), p_phrase, True, False, reg_ids, opener, m
        elif geo_stratum == "other_26_ufs":
            uf, m = self.next_other_uf_muni()
            use_reg = (seed % 2 == 0) and (uf in UF_DISCOURSE_OPENERS)
            opener, reg_ids = UF_DISCOURSE_OPENERS[uf] if use_reg else ("", [])
            phrase_style = seed % 3
            if phrase_style == 0:
                p_phrase = f"na cidade de {m['name']}/{uf}"
            elif phrase_style == 1:
                p_phrase = f"no município de {m['name']}/{uf}"
            else:
                p_phrase = f"em {m['name']}/{uf}"
            return uf, place_muni(m), p_phrase, False, bool(reg_ids), reg_ids, opener, m
        else:
            # National, macro-region, or whole-UF
            sub = seed % 4
            if sub == 0:
                return "BR", "any", "no Brasil", False, False, [], "", None
            elif sub == 1:
                regions = [
                    (1, "na região Norte"),
                    (2, "na região Nordeste"),
                    (3, "na região Sudeste"),
                    (4, "na região Sul"),
                    (5, "na região Centro-Oeste"),
                ]
                rid, rphrase = regions[seed % 5]
                return "BR", {"region": rid}, rphrase, (rid == 5), False, (["REG-MT-03"] if rid == 5 else []), "", None
            elif sub == 2:
                return "MT", place_uf("MT"), "no estado de Mato Grosso", True, False, ["REG-MT-03"], "", None
            else:
                uf = NON_MT_UFS[seed % len(NON_MT_UFS)]
                return uf, place_uf(uf), f"em {uf}", False, False, [], "", None

    def _make_item(
        self,
        item_num: int,
        text: str,
        register: str,
        gold: dict,
        rules: list[str],
        primary_family: str,
        business_decision: str,
        geo_stratum: str,
        uf: str,
        reg_ids: list[str],
        mt_context: bool,
        other_uf_regional: bool,
        scenario_family_id: str,
        surface_skeleton_id: str,
        novelty_reason,
        capability_requirement: str,
        source_refs: list[str],
        rationale: str,
        split: str,
        batch_id: str,
        context: dict | None = None,
    ) -> dict:
        final_text = self.ensure_unique_text(text, item_num, register)
        prov = "synthetic_regional_grounded" if (mt_context or other_uf_regional) else "synthetic_business_scenario"
        all_sources = list(dict.fromkeys(["RFB-CNPJ-2026-10", "IBGE-MUNI-202610"] + source_refs + [r for r in reg_ids if r.startswith("REG-") or r.startswith("EXT-")]))
        row = {
            "id": f"commercial-r09-{item_num:05d}",
            "text": final_text,
            "gold": gold,
            "rules": rules,
            "meta": {
                "category": primary_family,
                "primary_family": primary_family,
                "business_decision": business_decision,
                "geographic_stratum": geo_stratum,
                "uf": uf,
                "register": register,
                "regional_evidence_ids": reg_ids,
                "mt_context": mt_context,
                "other_uf_regional": other_uf_regional,
                "scenario_family_id": f"{split}::{scenario_family_id}",
                "surface_skeleton_id": f"{split}::{surface_skeleton_id}",
                "novelty_reason": novelty_reason,
                "capability_requirement": capability_requirement,
                "provenance_class": prov,
                "source_references": all_sources,
                "label_rationale": rationale,
                "rationale": rationale,
                "label_status": "authored_and_independently_verified",
                "split": split,
                "batch_id": batch_id,
                "authorship": "ai_synthetic_scenario",
                "human_validated": False,
                "language": "pt-BR",
            },
        }
        if context is not None:
            row["context"] = context
        return row

    def _gen_single_item(
        self,
        fam: str,
        unit_idx: int,
        local_idx: int,
        split: str,
        geo_stratum: str,
        register: str,
        batch_id: str,
        item_num: int,
    ) -> dict:
        seed = unit_idx * 1009 + local_idx * 37 + item_num
        uf, place_ir, place_phrase, mt_ctx, ouf_reg, reg_ids, opener, muni_obj = self._resolve_geo(geo_stratum, seed)
        s = self.pick_sector(seed)
        sname = s["sing"]
        splural = s["plural"]

        if fam == "market_size_density_competition":
            sub = local_idx % 5
            if sub == 0:
                # Active stock count with regional or business framing
                if split == "synthetic_dev":
                    text = f"{opener}Quantos estabelecimentos ativos de {sname} existem {place_phrase} para dimensionar a concorrência local?"
                elif split == "synthetic_eval":
                    text = f"{opener}Conte quantos estabelecimentos ativos de {sname} funcionam {place_phrase} no retrato atual."
                else:
                    # Post-nominal quantas tem (tests word-order generalization!)
                    text = f"{opener}Estabelecimentos ativos de {sname} {place_phrase}, quantos existem?"
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s))
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06", "SR-07"],
                    fam, "Size active market stock in target territory", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"mkt_stock_{sub}", f"COUNT/STOCK/SECTOR/PLACE/{sub}",
                    "regional_discourse_marker_or_postnominal_count" if (opener or split == "synthetic_challenge") else None,
                    "supported_aggregates", [], "Active establishment stock count for explicit sector and place.", split, batch_id
                )
            elif sub == 1:
                # Density per 10k inhabitants (2026 estimate or 2022 census)
                if place_ir == "any":
                    place_ir = place_uf("MT")
                    place_phrase = "no estado de Mato Grosso"
                    uf = "MT"
                    mt_ctx = True
                use_census = (local_idx % 2 == 1)
                pop_ref = {"ibge_census": 2022} if use_census else {"ibge_estimate": 2026}
                pop_txt = "pelo censo IBGE 2022" if use_census else "pela estimativa IBGE 2026"
                if split == "synthetic_dev":
                    text = f"{opener}Qual a densidade por 10 mil habitantes de estabelecimentos ativos de {sname} {place_phrase} {pop_txt}?"
                elif split == "synthetic_eval":
                    text = f"{opener}Informe a taxa por 10 mil habitantes de estabelecimentos ativos de {sname} {place_phrase} {pop_txt}."
                else:
                    text = f"{opener}Calcule a densidade por 10 mil habitantes para estabelecimentos ativos de {sname} {place_phrase} {pop_txt}."
                ir = base_ir({"kind": "count", "metric": {"per10k": pop_ref}}, place_ir, activity_sector(s))
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06", "SR-11"],
                    fam, "Evaluate market saturation per 10,000 inhabitants", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"mkt_per10k_{use_census}", f"COUNT/PER10K/SECTOR/PLACE/{use_census}",
                    "mt_or_regional_density_per10k", "supported_aggregates", [],
                    "Density per 10,000 inhabitants with explicit IBGE population reference.", split, batch_id
                )
            elif sub == 2:
                # Inhabitants per establishment
                if place_ir == "any":
                    place_ir = place_uf("MT")
                    place_phrase = "em Mato Grosso"
                    uf = "MT"
                    mt_ctx = True
                text = f"{opener}Quantos habitantes por estabelecimento ativo de {sname} existem {place_phrase} segundo a estimativa IBGE 2026?"
                ir = base_ir({"kind": "count", "metric": {"inhabitants_per": {"ibge_estimate": 2026}}}, place_ir, activity_sector(s))
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06", "SR-11"],
                    fam, "Measure inhabitants served per active establishment", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "mkt_inhab_per", "COUNT/INHAB_PER/SECTOR/PLACE",
                    "inhabitants_per_establishment_saturation", "supported_aggregates", [],
                    "Inhabitants per active establishment using IBGE 2026 estimate.", split, batch_id
                )
            elif sub == 3:
                # Ranking of municipalities within UF + population band filter
                target_uf = "MT" if uf in ("MT", "BR", "MULTI") else uf
                if target_uf == "MT":
                    mt_ctx = True
                k = [5, 10, 15][(local_idx // 5) % 3]
                pop_thresh = [20000, 30000, 50000][(local_idx // 3) % 3]
                text = f"{opener}Quais os top {k} municípios com mais estabelecimentos ativos de {sname} em {target_uf} considerando apenas cidades com mais de {pop_thresh} habitantes?"
                extra = {
                    "place_population": {"from": pop_thresh + 1, "to": None},
                    "population_ref": {"ibge_estimate": 2026},
                }
                ir = base_ir(
                    {"kind": "rank", "dim": "municipality", "metric": "stock", "dir": "desc", "k": k, "min_base": 0},
                    place_uf(target_uf), activity_sector(s), extra_scope=extra
                )
                gold = {"decision": "execute", "intent": "rank", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07", "SR-11"],
                    fam, "Rank municipalities in state filtered by minimum population", geo_stratum, target_uf, reg_ids, mt_ctx, ouf_reg,
                    "mkt_rank_popband", "RANK/MUNI/STOCK/POP_BAND",
                    "rank_with_population_band_filter", "supported_aggregates", [],
                    "Municipality ranking by active stock restricted to municipalities above population threshold.", split, batch_id
                )
            else:
                # Regional abundance / saturation metaphor minimal pair -> clarify!
                if local_idx % 2 == 0:
                    text = f"Dizem que estabelecimento de {sname} {place_phrase} é mato; isso procede na praça?"
                    reg_list = list(dict.fromkeys(reg_ids + ["reg_mt_e_mato", "REG-MT-03"]))
                else:
                    text = f"O mercado de {sname} {place_phrase} já tá saturado ou ainda cabe mais uma operação?"
                    reg_list = list(dict.fromkeys(reg_ids + ["biz_amb_saturation", "REG-BR-01"]))
                gold = {
                    "decision": "clarify",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Ask whether the user wants active establishment count, density per 10k inhabitants, or inhabitants per establishment.",
                }
                return self._make_item(
                    item_num, text, register, gold, ["SR-09", "SR-10"],
                    fam, "Clarify subjective market saturation or regional abundance metaphor", geo_stratum, uf, reg_list, True, ouf_reg,
                    "mkt_saturation_clarify", "CLARIFY/SATURATION_ABUNDANCE",
                    "regional_abundance_or_saturation_metaphor_minimal_pair", "clarify_ambiguous_quality", [],
                    "Abundance/saturation predicates ('é mato', 'tá saturado', 'cabe mais uma') require clarifying the cadastral metric.", split, batch_id
                )

        if fam == "prospecting_qualification_territory":
            # Lists with 2-filter and 3-filter combinations
            sub = local_idx % 7
            limit = [10, 15, 20, 25, 30, 50][local_idx % 6]
            contact_modes = [
                ("phone", "com telefone cadastrado"),
                ("email", "com e-mail cadastrado"),
                ("both", "com telefone e e-mail cadastrados"),
            ]
            c_code, c_phrase = contact_modes[local_idx % 3]
            if sub == 0:
                # 3-filter: contact + head_office + porte
                text = f"{opener}Liste {limit} estabelecimentos ativos de {sname} {place_phrase} que sejam matriz, microempresa e {c_phrase}."
                extra = {"contact": c_code, "head_office": "yes", "porte": ["micro"]}
                order = "cnpj"
            elif sub == 1:
                # 3-filter: contact + mei=no + simples=yes
                text = f"{opener}Liste {limit} estabelecimentos ativos de {sname} {place_phrase} optantes pelo Simples Nacional, que não sejam MEI e {c_phrase}."
                extra = {"contact": c_code, "simples": "yes", "mei": "no"}
                order = "cnpj"
            elif sub == 2:
                # 3-filter: contact + head_office + capital range, ordered by capital desc
                cap_reais = [50000, 100000, 250000, 500000][local_idx % 4]
                text = f"{opener}Liste {limit} estabelecimentos ativos de {sname} {place_phrase} que sejam matriz, {c_phrase} e com capital social acima de {cap_reais} reais, ordenados pelo maior capital social."
                extra = {"contact": c_code, "head_office": "yes", "capital": {"from": cap_reais * 100 + 1, "to": None}}
                order = "capital_desc"
            elif sub == 3:
                # 3-filter: contact + opened in 2025 + porte=small (pequeno porte)
                text = f"{opener}Liste {limit} estabelecimentos ativos de {sname} {place_phrase} de pequeno porte, {c_phrase} e com início de atividade em 2025, dos mais recentes primeiro."
                extra = {
                    "contact": c_code,
                    "porte": ["small"],
                    "opened": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)},
                }
                order = "newest_first"
            elif sub == 4:
                # 3-filter: contact + natureza 2062 (LTDA) + head_office
                text = f"{opener}Liste {limit} estabelecimentos ativos de {sname} {place_phrase} que sejam matriz, sociedade empresária limitada e {c_phrase}, dos mais antigos primeiro."
                extra = {"contact": c_code, "head_office": "yes", "natureza": [{"code": 2062}]}
                order = "oldest_first"
            elif sub == 5:
                # Paginated list (page 2 or 3) + contact + mei=yes
                page = 2 if (local_idx % 2 == 0) else 3
                text = f"{opener}Traga a página {page} e liste {limit} estabelecimentos ativos de {sname} {place_phrase} optantes pelo MEI e {c_phrase}."
                extra = {"contact": c_code, "mei": "yes"}
                ir = base_ir({"kind": "list", "order": "cnpj", "limit": limit}, place_ir, activity_sector(s), page=page, extra_scope=extra)
                gold = {"decision": "execute", "intent": "list", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07", "SR-12"],
                    fam, "Paginate qualified B2B/B2C prospecting list with contact and tax filters", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"prosp_page_{page}", f"LIST/PAGE_{page}/CONTACT_MEI",
                    "multi_filter_paginated_prospecting_list", "supported_engine", [],
                    "Prospecting list with explicit page, limit, MEI filter, and contact presence.", split, batch_id
                )
            else:
                # Territory qualification count: 3-filter (head_office + porte + simples)
                text = f"{opener}Quantos estabelecimentos ativos de {sname} existem {place_phrase} que são matriz, de pequeno porte e optantes pelo Simples Nacional?"
                extra = {"head_office": "yes", "porte": ["small"], "simples": "yes"}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06", "SR-07"],
                    fam, "Count qualified establishments combining head office, size, and Simples regime", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "prosp_count_triple", "COUNT/STOCK/TRIPLE_HQ_PORTE_SIMPLES",
                    "three_filter_combination_hq_porte_simples", "supported_aggregates", [],
                    "Three-filter intersection count (head_office + porte + simples).", split, batch_id
                )

            ir = base_ir({"kind": "list", "order": order, "limit": limit}, place_ir, activity_sector(s), extra_scope=extra)
            gold = {"decision": "execute", "intent": "list", "ir": ir, "act": "new_query"}
            return self._make_item(
                item_num, text, register, gold, ["SR-02", "SR-03", "SR-06", "SR-07", "SR-12"],
                fam, "Build qualified commercial prospecting list with triple filter intersection", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                f"prosp_triple_{sub}", f"LIST/{order.upper()}/TRIPLE_{sub}",
                f"three_filter_prospecting_combination_{sub}", "supported_engine", [],
                "Prospecting list with three explicit cadastral filters and deterministic sort order.", split, batch_id
            )

        if fam == "historical_stock_flows_seasonality":
            sub = local_idx % 5
            if sub == 0:
                # Cohort closure query: closed in 2026 AND opened in 2025 (or 2024)
                open_year = 2025 if (local_idx % 2 == 0) else 2024
                text = f"{opener}Quantos estabelecimentos de {sname} tiveram baixa cadastral em 2026 com início de atividade em {open_year} {place_phrase}?"
                extra = {"opened": {"from": day_from_ymd(open_year, 1, 1), "to": day_from_ymd(open_year, 12, 31)}}
                period = {"days": {"from": day_from_ymd(2026, 1, 1), "to": ANCHOR_DAY}}
                ir = base_ir(
                    {"kind": "count", "metric": {"flow": "closures"}},
                    place_ir, activity_sector(s), status=["closed"], period=period, extra_scope=extra
                )
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-05", "SR-06", "SR-10"],
                    fam, "Measure cohort survival/closure of establishments opened in a specific year", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"hist_cohort_closure_{open_year}", "COUNT/CLOSURES/OPENED_COHORT",
                    "cohort_closure_with_explicit_opened_filter", "supported_engine", [],
                    "Closure count in 2026 restricted to cohort with opening date in the specified year.", split, batch_id
                )
            elif sub == 1:
                # Historical monthly stock snapshot (competences 298..=320)
                hist_ym = [
                    (2024, 11), (2024, 12), (2025, 1), (2025, 3), (2025, 6),
                    (2025, 9), (2025, 12), (2026, 3), (2026, 6), (2026, 9),
                ][local_idx % 10]
                comp_id = comp_from_ym(hist_ym[0], hist_ym[1])
                ym_str = f"{hist_ym[0]:04d}-{hist_ym[1]:02d}"
                text = f"{opener}Quantos estabelecimentos de {sname} estavam ativos {place_phrase} na competência {ym_str}?"
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), competence=comp_id)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-05", "SR-06"],
                    fam, "Inspect historical monthly active stock at past snapshot competence", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"hist_stock_{ym_str}", "COUNT/STOCK/HISTORICAL_COMP",
                    None, "supported_aggregates", [],
                    f"Historical active stock count at competence {ym_str}.", split, batch_id
                )
            elif sub == 2:
                # Active stock change between two competences
                from_ym = [(2024, 11), (2025, 1), (2025, 6), (2025, 10)][local_idx % 4]
                from_comp = comp_from_ym(from_ym[0], from_ym[1])
                from_str = f"{from_ym[0]:04d}-{from_ym[1]:02d}"
                use_pct = (local_idx % 2 == 1)
                metric_phrase = "a variação percentual do estoque ativo" if use_pct else "a variação absoluta do estoque ativo"
                text = f"{opener}Qual foi {metric_phrase} de estabelecimentos de {sname} {place_phrase} entre {from_str} e 2026-10?"
                ir = base_ir(
                    {"kind": "count", "metric": {"active_change": {"pct": use_pct}}},
                    place_ir, activity_sector(s),
                    period={"competences": {"from": from_comp, "to": CURRENT_COMP}}
                )
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-03", "SR-05", "SR-06"],
                    fam, "Compute active stock change between historical and current competences", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"hist_active_change_{use_pct}", "COUNT/ACTIVE_CHANGE/COMP_RANGE",
                    "historical_competence_stock_delta", "supported_aggregates", [],
                    "Active stock delta between two monthly snapshot competences.", split, batch_id
                )
            elif sub == 3:
                # Quarterly or semester registration flow
                periods = [
                    ("no 1 trimestre de 2025", day_from_ymd(2025, 1, 1), day_from_ymd(2025, 3, 31)),
                    ("no 2 trimestre de 2025", day_from_ymd(2025, 4, 1), day_from_ymd(2025, 6, 30)),
                    ("no 3 trimestre de 2025", day_from_ymd(2025, 7, 1), day_from_ymd(2025, 9, 30)),
                    ("no 4 trimestre de 2025", day_from_ymd(2025, 10, 1), day_from_ymd(2025, 12, 31)),
                    ("no primeiro semestre de 2025", day_from_ymd(2025, 1, 1), day_from_ymd(2025, 6, 30)),
                    ("no segundo semestre de 2025", day_from_ymd(2025, 7, 1), day_from_ymd(2025, 12, 31)),
                ]
                p_txt, d_from, d_to = periods[local_idx % len(periods)]
                text = f"{opener}Quantos estabelecimentos de {sname} tiveram registros de início de atividade {p_txt} {place_phrase}?"
                ir = base_ir(
                    {"kind": "count", "metric": {"flow": "registrations"}},
                    place_ir, activity_sector(s), status=ALL_STATUS,
                    period={"days": {"from": d_from, "to": d_to}}
                )
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-05", "SR-06"],
                    fam, "Track seasonal quarterly/semester business registrations", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "hist_seasonal_reg", "COUNT/REGISTRATIONS/QUARTER_SEMESTER",
                    "seasonal_quarter_or_semester_flow", "supported_aggregates", [],
                    "Registration flow count over explicit quarter or semester window.", split, batch_id
                )
            else:
                # Monthly trend of registrations, closures, or net_registrations in 2025
                flow_kinds = [
                    ("registrations", "registros de início de atividade", ALL_STATUS),
                    ("closures", "baixas cadastrais", ["closed"]),
                    ("net_registrations", "saldo entre aberturas e baixas cadastrais", ALL_STATUS),
                ]
                f_code, f_txt, f_status = flow_kinds[local_idx % 3]
                text = f"{opener}Mostre a evolução mensal em 2025 de {f_txt} de estabelecimentos de {sname} {place_phrase}."
                ir = base_ir(
                    {"kind": "trend", "metric": f_code, "grain": "month"},
                    place_ir, activity_sector(s), status=f_status,
                    period={"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                )
                gold = {"decision": "execute", "intent": "trend", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-05", "SR-06"],
                    fam, "Analyze monthly seasonality of registrations, closures, or net flow", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"hist_trend_{f_code}", f"TREND/MONTH/{f_code.upper()}",
                    "monthly_net_or_event_flow_trend" if f_code == "net_registrations" else None,
                    "supported_aggregates", [],
                    "Monthly flow trend over 2025.", split, batch_id
                )

        if fam == "geography_catchments_comparisons":
            sub = local_idx % 5
            if sub == 0 and muni_obj is not None:
                # Contiguous neighbor catchment (1 or 2 steps), optionally constrained to UF
                steps = 1 if (local_idx % 2 == 0) else 2
                text = f"{opener}Quantos estabelecimentos ativos de {sname} existem na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos municípios vizinhos em até {steps} passos?"
                near_place = {"near": {"center": muni_obj["id"], "rule": {"neighbours": steps}}}
                ir = base_ir({"kind": "count", "metric": "stock"}, near_place, activity_sector(s))
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Size contiguous municipal catchment for regional sales route", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"geo_neighbours_{steps}", f"COUNT/STOCK/NEAR_NEIGHBOURS_{steps}",
                    "contiguous_municipality_catchment", "supported_aggregates", [],
                    "Active stock in center municipality plus contiguous neighbor ring.", split, batch_id
                )
            elif sub == 1 and muni_obj is not None:
                # Radius catchment in km
                km = [50, 80, 100, 120, 150][local_idx % 5]
                text = f"{opener}Quantos estabelecimentos ativos de {sname} existem na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos municípios próximos em um raio de {km} km?"
                near_place = {"near": {"center": muni_obj["id"], "rule": {"within_km": km}}}
                ir = base_ir({"kind": "count", "metric": "stock"}, near_place, activity_sector(s))
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Size radial logistics catchment in kilometers around hub city", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"geo_radius_{km}", f"COUNT/STOCK/NEAR_KM_{km}",
                    "radial_km_catchment_hub", "supported_aggregates", [],
                    "Active stock within centroid distance radius in kilometers.", split, batch_id
                )
            elif sub == 2:
                # 2-city comparison
                if geo_stratum.startswith("mt_"):
                    m1 = muni_obj or self.next_mt_muni(geo_stratum, seed)
                    m2 = self.next_mt_muni(geo_stratum, seed + 1)
                    if m2["id"] == m1["id"]:
                        m2 = self.next_mt_muni("mt_metropolitan", seed + 2)
                elif geo_stratum == "other_26_ufs":
                    m1 = muni_obj or self.uf_munis[uf][0]
                    uf_list = self.uf_munis[uf]
                    m2 = uf_list[(seed + 1) % len(uf_list)]
                    if m2["id"] == m1["id"]:
                        m2 = uf_list[(seed + 2) % len(uf_list)] if len(uf_list) > 1 else self.mt_strata["mt_north_agro_industrial"][0]
                else:
                    m1 = self.mt_strata["mt_north_agro_industrial"][seed % len(self.mt_strata["mt_north_agro_industrial"])]
                    other_uf = NON_MT_UFS[seed % len(NON_MT_UFS)]
                    m2 = self.uf_munis[other_uf][seed % len(self.uf_munis[other_uf])]
                members = [{"municipality": mid} for mid in sorted({m1["id"], m2["id"]})]
                use_density = (local_idx % 2 == 1)
                metric = {"per10k": {"ibge_estimate": 2026}} if use_density else "stock"
                m_phrase = "a densidade por 10 mil habitantes de estabelecimentos ativos" if use_density else "a quantidade de estabelecimentos ativos"
                text = f"{opener}Compare {m_phrase} de {sname} entre a cidade de {m1['name']}/{m1['uf']} e a cidade de {m2['name']}/{m2['uf']}."
                ir = base_ir({"kind": "compare", "dim": "municipality", "members": members, "metric": metric}, "any", activity_sector(s))
                gold = {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07", "SR-11"],
                    fam, "Compare two commercial hub municipalities on stock or population-adjusted density", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"geo_compare_muni_{use_density}", f"COMPARE/MUNI/{'PER10K' if use_density else 'STOCK'}",
                    "corridor_municipality_density_comparison" if use_density else None,
                    "supported_aggregates", [],
                    "Municipality comparison on explicit metric and activity.", split, batch_id
                )
            elif sub == 3:
                # State-level count with municipality exclusion
                if uf == "MT" or geo_stratum.startswith("mt_"):
                    ex_m = muni_obj or self.mt_strata["mt_metropolitan"][0]
                    target_uf = "MT"
                    mt_ctx = True
                elif geo_stratum == "other_26_ufs":
                    ex_m = muni_obj or self.uf_munis[uf][0]
                    target_uf = uf
                else:
                    target_uf = NON_MT_UFS[seed % len(NON_MT_UFS)]
                    ex_m = self.uf_munis[target_uf][0]
                text = f"{opener}Quantos estabelecimentos ativos de {sname} existem em {target_uf}, exceto os da cidade de {ex_m['name']}/{target_uf}?"
                extra = {"exclude": {"municipalities": [ex_m["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_uf(target_uf), activity_sector(s), extra_scope=extra)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Size interior state market excluding hub municipality", geo_stratum, target_uf, reg_ids, mt_ctx, ouf_reg,
                    "geo_exclude_muni", "COUNT/STOCK/UF_EXCLUDE_MUNI",
                    "state_interior_excluding_hub_city", "supported_aggregates", [],
                    "UF stock count excluding the specified municipality.", split, batch_id
                )
            else:
                # UF-to-UF comparison (MT vs other UF)
                other_uf = uf if geo_stratum == "other_26_ufs" else NON_MT_UFS[local_idx % len(NON_MT_UFS)]
                uf1 = "MT" if (geo_stratum.startswith("mt_") or uf == "MT") else other_uf
                uf2 = "GO" if uf1 == "MT" and other_uf == "MT" else (other_uf if uf1 == "MT" else "MT")
                members = [{"uf": uid} for uid in sorted({UF_ID[uf1], UF_ID[uf2]})]
                text = f"{opener}Compare a quantidade de estabelecimentos ativos de {sname} entre o estado de {UF_NAME[uf1]} e o estado de {UF_NAME[uf2]}."
                ir = base_ir({"kind": "compare", "dim": "uf", "members": members, "metric": "stock"}, "any", activity_sector(s))
                gold = {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"}
                item_uf = uf if geo_stratum == "other_26_ufs" else ("MT" if uf == "MT" else "MULTI")
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Compare two states on active establishments in sector", geo_stratum, item_uf, reg_ids, True, ouf_reg,
                    "geo_compare_ufs", "COMPARE/UF/STOCK",
                    "cross_state_corridor_comparison", "supported_aggregates", [],
                    "State-to-state comparison on active establishment stock.", split, batch_id
                )

        if fam == "corporate_structure_hq_branches_identity":
            sub = local_idx % 5
            if sub == 0:
                # HeadOffices unit count (quantas matrizes ativas)
                text = f"{opener}Quantas matrizes ativas de {sname} existem {place_phrase}?"
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), unit="head_offices")
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06"],
                    fam, "Count active head offices (company headquarters) in territory", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "corp_head_offices", "COUNT/STOCK/UNIT_HEAD_OFFICES",
                    None, "supported_aggregates", [],
                    "Head-office count uses unit='head_offices'.", split, batch_id
                )
            elif sub == 1:
                # Branches only (somente filiais ativas)
                text = f"{opener}Quantas filiais ativas de {sname} existem {place_phrase}?"
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope={"head_office": "no"})
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06"],
                    fam, "Count active branch establishments in territory", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "corp_branches_only", "COUNT/STOCK/BRANCHES_ONLY",
                    None, "supported_aggregates", [],
                    "Branch count sets head_office='no' on establishment unit.", split, batch_id
                )
            elif sub == 2:
                # Breakdown by head_office or status
                if local_idx % 2 == 0:
                    text = f"{opener}Distribua os estabelecimentos ativos de {sname} {place_phrase} por matriz e filial, com percentual."
                    ir = base_ir({"kind": "breakdown", "dim": "head_office", "metric": "share"}, place_ir, activity_sector(s))
                else:
                    text = f"{opener}Distribua os estabelecimentos de {sname} {place_phrase} em todas as situações por situação cadastral."
                    ir = base_ir({"kind": "breakdown", "dim": "status", "metric": "stock"}, place_ir, activity_sector(s), status=ALL_STATUS)
                gold = {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06"],
                    fam, "Break down corporate structure by head office/branch or cadastral status", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"corp_breakdown_{local_idx % 2}", "BREAKDOWN/HQ_OR_STATUS",
                    "corporate_structure_breakdown", "supported_aggregates", [],
                    "Breakdown by head_office or cadastral status.", split, batch_id
                )
            elif sub == 3:
                # Valid CNPJ lookup partitioned by split so no CNPJ root crosses splits!
                pool = self.cnpjs["mt"] if (uf == "MT" or geo_stratum.startswith("mt_")) else self.cnpjs["other"]
                # Partition pool by split: 0..400 dev, 400..520 eval, 520..600 challenge
                if split == "synthetic_dev":
                    c_entry = pool[(unit_idx * 4 + (local_idx // 5)) % 400]
                elif split == "synthetic_eval":
                    c_entry = pool[400 + ((unit_idx * 2 + (local_idx // 5)) % 120)]
                else:
                    c_entry = pool[520 + ((unit_idx + (local_idx // 5)) % 80)]
                use_root = (local_idx % 2 == 1)
                cid = c_entry["root"] if use_root else c_entry["full"]
                label_kind = "a raiz de CNPJ" if use_root else "o CNPJ"
                text = f"{opener}Consulte {label_kind} {cid} na base cadastral para conferência societária."
                ir = base_ir({"kind": "lookup", "id": cid}, "any", "any", status=ALL_STATUS)
                gold = {"decision": "execute", "intent": "lookup", "ir": ir, "act": "new_query"}
                item_uf = uf if geo_stratum == "other_26_ufs" else c_entry["uf"]
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-13"],
                    fam, "Verify exact CNPJ or 8-digit company root in snapshot", geo_stratum, item_uf, reg_ids, mt_ctx, ouf_reg,
                    f"corp_lookup_{use_root}", "LOOKUP/VALID_CNPJ",
                    "verified_mt_or_national_cnpj_lookup", "supported_engine", [],
                    "Exact lookup of verified CNPJ or 8-digit root.", split, batch_id
                )
            else:
                # Non-active status query (suspensos, inaptos, baixados, não ativos)
                st_variants = [
                    (["suspended", "unfit", "closed"], "que não estão ativos"),
                    (["suspended"], "suspensos"),
                    (["unfit"], "inaptos"),
                    (["closed"], "baixados"),
                ]
                st_val, st_phrase = st_variants[(local_idx // 5) % 4]
                text = f"{opener}Quantos estabelecimentos de {sname} {st_phrase} constam no cadastro {place_phrase}?"
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), status=st_val)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-03", "SR-06", "SR-07"],
                    fam, "Audit non-active cadastral statuses in territory", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "corp_non_active_status", "COUNT/STOCK/NON_ACTIVE_STATUS",
                    "explicit_non_active_cadastral_status_audit", "supported_aggregates", [],
                    "Count of establishments with explicit non-active cadastral status.", split, batch_id
                )

        if fam == "supply_chain_complementary":
            sub = local_idx % 5
            s1, s2 = self.supply_pairs[(unit_idx * 11 + local_idx) % len(self.supply_pairs)]
            if sub == 0:
                # Compare upstream vs downstream sector in the same place (stock)
                members = [{"sector": sid} for sid in sorted({s1["id"], s2["id"]})]
                text = f"{opener}Para mapear a cadeia produtiva {place_phrase}: compare a quantidade de estabelecimentos ativos entre {s1['sing']} e {s2['sing']}."
                ir = base_ir({"kind": "compare", "dim": "sector", "members": members, "metric": "stock"}, place_ir, "any")
                gold = {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Compare upstream and downstream supply-chain sectors in same territory", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "supply_compare_stock", "COMPARE/SECTOR/STOCK",
                    "supply_chain_upstream_downstream_sector_comparison", "supported_aggregates", [],
                    "Sector-dimension comparison between two complementary supply-chain activities in the same place.", split, batch_id
                )
            elif sub == 1:
                # Compare upstream vs downstream sector in registrations in 2025
                members = [{"sector": sid} for sid in sorted({s1["id"], s2["id"]})]
                text = f"{opener}Na cadeia comercial {place_phrase}: compare os registros de início de atividade em 2025 entre estabelecimentos de {s1['sing']} e de {s2['sing']}."
                period = {"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                ir = base_ir(
                    {"kind": "compare", "dim": "sector", "members": members, "metric": {"flow": "registrations"}},
                    place_ir, "any", status=ALL_STATUS, period=period
                )
                gold = {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-06", "SR-07"],
                    fam, "Compare new business entry across two complementary supply-chain links", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "supply_compare_reg", "COMPARE/SECTOR/REGISTRATIONS",
                    "supply_chain_registration_flow_comparison", "supported_aggregates", [],
                    "Sector comparison on 2025 registration flows.", split, batch_id
                )
            elif sub == 2:
                # Combined union of two complementary sectors (including secondary CNAE on half)
                use_any_cnae = (local_idx % 2 == 1)
                cnae_scope = "any" if use_any_cnae else "main"
                sec_phrase = "incluindo CNAE secundário" if use_any_cnae else "no CNAE principal"
                text = f"{opener}Quantos estabelecimentos ativos de {s1['sing']} e de {s2['sing']} ({sec_phrase}) existem {place_phrase}?"
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_two_sectors(s1, s2, cnae_scope))
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Count combined supply-chain ecosystem across complementary sectors", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"supply_union_{cnae_scope}", f"COUNT/STOCK/TWO_SECTORS_{cnae_scope.upper()}",
                    "multi_sector_complementary_union_with_cnae_scope",
                    "supported_engine" if use_any_cnae else "supported_aggregates", [],
                    "Combined count across two complementary sectors with explicit primary/secondary CNAE scope.", split, batch_id
                )
            elif sub == 3:
                # Top sectors ranking in territory
                k = [5, 10][local_idx % 2]
                text = f"{opener}Quais os top {k} setores com mais estabelecimentos ativos {place_phrase}?"
                ir = base_ir({"kind": "rank", "dim": "sector", "metric": "stock", "dir": "desc", "k": k, "min_base": 0}, place_ir, "any")
                gold = {"decision": "execute", "intent": "rank", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Identify dominant economic sectors in target territory", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "supply_rank_sectors", "RANK/SECTOR/STOCK",
                    "territorial_sector_specialization_ranking", "supported_aggregates", [],
                    "Ranking of curated sectors by active establishment count in territory.", split, batch_id
                )
            else:
                # Breakdown by CNAE section or division in territory
                use_div = (local_idx % 2 == 1)
                dim_val = {"cnae": "division" if use_div else "section"}
                dim_txt = "por divisão CNAE" if use_div else "por seção CNAE"
                text = f"{opener}Distribua os estabelecimentos ativos {place_phrase} {dim_txt}."
                ir = base_ir({"kind": "breakdown", "dim": dim_val, "metric": "stock"}, place_ir, "any")
                gold = {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-07"],
                    fam, "Map full CNAE section or division profile of local economy", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"supply_cnae_breakdown_{use_div}", "BREAKDOWN/CNAE_LEVEL",
                    "cnae_hierarchy_structural_breakdown", "supported_aggregates", [],
                    "Breakdown of active establishments across CNAE sections or divisions.", split, batch_id
                )

        if fam == "cross_variable_correlation":
            # 60% supported cadastral/IBGE-pop cross-variable queries, 40% unsupported external/statistical fail-closed (reject)
            sub = local_idx % 5
            if sub == 0:
                # Supported: Rank municipalities by density per 10k inhabitants in a population band
                target_uf = "MT" if (uf in ("MT", "BR", "MULTI") or geo_stratum.startswith("mt_")) else uf
                if target_uf == "MT":
                    mt_ctx = True
                pop_min = [25000, 40000, 60000][local_idx % 3]
                text = f"{opener}Para cruzar porte populacional do IBGE e presença setorial: quais os top 10 municípios com maior densidade por 10 mil habitantes de {sname} em {target_uf} entre cidades com mais de {pop_min} habitantes?"
                extra = {
                    "place_population": {"from": pop_min + 1, "to": None},
                    "population_ref": {"ibge_estimate": 2026},
                }
                ir = base_ir(
                    {"kind": "rank", "dim": "municipality", "metric": {"per10k": {"ibge_estimate": 2026}}, "dir": "desc", "k": 10, "min_base": 0},
                    place_uf(target_uf), activity_sector(s), extra_scope=extra
                )
                gold = {"decision": "execute", "intent": "rank", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-06", "SR-11"],
                    fam, "Cross-reference municipal population threshold with per-10k sector density", geo_stratum, target_uf, reg_ids, mt_ctx, ouf_reg,
                    "corr_supported_pop_density_rank", "RANK/MUNI/PER10K/POP_BAND",
                    "cross_variable_population_band_and_per10k_density", "supported_aggregates", [],
                    "Supported cross-variable analysis combining IBGE population band filter with per-10k density ranking.", split, batch_id
                )
            elif sub == 1:
                # Supported: Net registration flow balance (registrations minus closures) in 2025
                text = f"{opener}Para cruzar fluxo de entrada e saída cadastral: qual o saldo entre aberturas e baixas de estabelecimentos de {sname} em 2025 {place_phrase}?"
                period = {"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                ir = base_ir(
                    {"kind": "count", "metric": {"flow": "net_registrations"}},
                    place_ir, activity_sector(s), status=ALL_STATUS, period=period
                )
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-05", "SR-06"],
                    fam, "Compare registration inflows against closure outflows via net flow balance", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "corr_supported_net_flow", "COUNT/NET_REGISTRATIONS/YEAR",
                    "cross_variable_inflow_outflow_net_balance", "supported_aggregates", [],
                    "Net registration balance (registrations minus closures) over 2025.", split, batch_id
                )
            elif sub == 2:
                # Supported: Breakdown of active establishments by porte or MEI with share percentage
                use_mei = (local_idx % 2 == 1)
                dim_code = "mei" if use_mei else "porte"
                dim_txt = "por opção pelo MEI" if use_mei else "por porte cadastral"
                text = f"{opener}Para relacionar perfil tributário/porte e presença comercial: distribua os estabelecimentos ativos de {sname} {place_phrase} {dim_txt}, com percentual."
                ir = base_ir({"kind": "breakdown", "dim": dim_code, "metric": "share"}, place_ir, activity_sector(s))
                gold = {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06"],
                    fam, "Analyze joint distribution of sector activity across company size or MEI regime", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"corr_supported_share_{dim_code}", f"BREAKDOWN/{dim_code.upper()}/SHARE",
                    "cross_variable_sector_by_size_or_mei_share", "supported_aggregates", [],
                    "Percentage share breakdown of sector establishments by size or MEI status.", split, batch_id
                )
            elif sub == 3:
                # Unsupported: External IBGE dataset (CEMPRE, PAM, or PPM) -> fail closed (reject)!
                ext_mode = local_idx % 3
                if ext_mode == 0:
                    text = f"{opener}Qual a correlação entre a produção de soja em toneladas da PAM do IBGE e o total de estabelecimentos ativos de {sname} {place_phrase}?"
                    ext_src = ["EXT-IBGE-PAM"]
                    beh = "Reject: IBGE PAM agricultural production tonnage and statistical correlation are not in the loaded cadastral/population artifacts."
                elif ext_mode == 1:
                    text = f"{opener}Cruze os dados do CEMPRE do IBGE de pessoal ocupado assalariado e salário médio com os estabelecimentos ativos de {sname} {place_phrase}."
                    ext_src = ["EXT-IBGE-CEMPRE"]
                    beh = "Reject: IBGE CEMPRE headcount and wage mass tables are not integrated into the runtime snapshot/aggregate artifacts."
                else:
                    text = f"{opener}Qual a razão entre o efetivo de rebanho bovino da PPM do IBGE e o número de estabelecimentos ativos de {sname} {place_phrase}?"
                    ext_src = ["EXT-IBGE-PPM"]
                    beh = "Reject: IBGE PPM livestock herd counts are not loaded in the cadastral/population runtime."
                gold = {"decision": "reject", "intent": None, "ir": None, "expected_behavior": beh}
                return self._make_item(
                    item_num, text, register, gold, ["SR-01", "SR-09", "SR-10"],
                    fam, "Fail closed when asked for unintegrated external IBGE CEMPRE/PAM/PPM series", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"corr_reject_external_{ext_mode}", f"REJECT/EXTERNAL_IBGE_{ext_mode}",
                    "fail_closed_external_cempre_pam_ppm_correlation", "reject_external_cempre_pam_ppm", ext_src,
                    beh, split, batch_id
                )
            else:
                # Unsupported: Statistical regression / Pearson r / R^2 / elasticity -> fail closed (reject)!
                stat_mode = local_idx % 2
                if stat_mode == 0:
                    text = f"{opener}Calcule o coeficiente de correlação de Pearson e o p-valor entre capital social médio e população nos estabelecimentos de {sname} {place_phrase}."
                else:
                    text = f"{opener}Rode uma regressão linear com R2 entre a taxa de abertura de {sname} e a renda per capita {place_phrase}."
                beh = "Reject: statistical regression, Pearson correlation coefficients, p-values, and municipal income series are outside supported query operations."
                gold = {"decision": "reject", "intent": None, "ir": None, "expected_behavior": beh}
                return self._make_item(
                    item_num, text, register, gold, ["SR-01", "SR-09", "SR-10"],
                    fam, "Fail closed on unsupported statistical regression or Pearson correlation requests", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"corr_reject_stat_{stat_mode}", "REJECT/STATISTICAL_CORRELATION",
                    "fail_closed_statistical_correlation_or_regression", "reject_statistical_correlation_or_forecast", [],
                    beh, split, batch_id
                )

        if fam == "source_quality_provenance_uncertainty":
            sub = local_idx % 6
            if sub == 0:
                # Meta: data_date or sources
                use_sources = (local_idx % 2 == 1)
                topic = "sources" if use_sources else "data_date"
                if use_sources:
                    text = f"{opener}Quais fontes oficiais de dados são utilizadas para consultar estabelecimentos de {sname} {place_phrase}?"
                else:
                    text = f"{opener}De que data e competência são os dados usados para pesquisar {sname} {place_phrase}?"
                ir = base_ir({"kind": "meta", "topic": topic}, place_ir, activity_sector(s), status=ALL_STATUS)
                gold = {"decision": "execute", "intent": "meta", "ir": ir}
                return self._make_item(
                    item_num, text, register, gold, ["SR-01", "SR-14"],
                    fam, "Verify data provenance, official sources, and snapshot reference date", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"prov_meta_{topic}", f"META/{topic.upper()}",
                    None, "supported_static", [],
                    f"Meta query for {topic} preserving mentioned place and sector scope.", split, batch_id
                )
            elif sub == 1:
                # Meta: cnaes or methodology
                use_meth = (local_idx % 2 == 1)
                topic = "methodology" if use_meth else "cnaes"
                if use_meth:
                    text = f"{opener}Qual a metodologia usada para contar estabelecimentos ativos e baixas de {sname} {place_phrase}?"
                else:
                    text = f"{opener}Quais CNAEs entram na pesquisa de {sname} {place_phrase}?"
                ir = base_ir({"kind": "meta", "topic": topic}, place_ir, activity_sector(s), status=ALL_STATUS)
                gold = {"decision": "execute", "intent": "meta", "ir": ir}
                return self._make_item(
                    item_num, text, register, gold, ["SR-01", "SR-07", "SR-14"],
                    fam, "Audit CNAE mapping and counting methodology before commercial decision", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"prov_meta_{topic}", f"META/{topic.upper()}",
                    None, "supported_static", [],
                    f"Meta query for {topic}.", split, batch_id
                )
            elif sub == 2:
                # Homonym municipality without UF -> clarify (SR-08)
                hname = self.homonym_names[(unit_idx + local_idx) % len(self.homonym_names)]
                text = f"Quantos estabelecimentos ativos de {sname} existem na cidade de {hname}?"
                gold = {
                    "decision": "clarify",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": f"Clarify which UF is intended for homonymous municipality {hname}.",
                }
                item_uf = uf if geo_stratum == "other_26_ufs" else "UNRESOLVED"
                return self._make_item(
                    item_num, text, register, gold, ["SR-08"],
                    fam, "Require UF disambiguation for homonymous Brazilian/MT municipality", geo_stratum, item_uf, reg_ids,
                    hname in {"Água Boa", "Canarana", "Várzea Grande", "Pedra Preta", "Rio Branco", "Santa Terezinha", "Nova Olímpia", "General Carneiro", "Novo Santo Antônio"},
                    ouf_reg, "prov_clarify_homonym", "CLARIFY/HOMONYM_MUNI",
                    "mt_and_national_homonym_municipality_guard", "clarify_homonym", [],
                    "Homonymous municipality without UF or session context requires clarification.", split, batch_id
                )
            elif sub == 3:
                # Ambiguous activity term ('revenda agro', 'empresa do agro', 'quebra-torto', 'loja') -> clarify
                amb_mode = local_idx % 3
                if amb_mode == 0:
                    text = f"{opener}Quantas revendas agro ativas existem {place_phrase}?"
                    r_ids = list(dict.fromkeys(reg_ids + ["biz_amb_revenda_agro"]))
                elif amb_mode == 1:
                    text = f"{opener}Quantos estabelecimentos ativos de quebra-torto existem {place_phrase}?"
                    r_ids = list(dict.fromkeys(reg_ids + ["reg_mt_quebra_torto", "REG-MT-03"]))
                else:
                    text = f"{opener}Quantas lojas ativas existem {place_phrase}?"
                    r_ids = reg_ids
                gold = {
                    "decision": "clarify",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Clarify which specific CNAE sector or activity is intended.",
                }
                return self._make_item(
                    item_num, text, register, gold, ["SR-07", "SR-09"],
                    fam, "Clarify ambiguous commercial or regional activity umbrella term", geo_stratum, uf, r_ids, True if amb_mode < 2 else mt_ctx, ouf_reg,
                    f"prov_clarify_activity_{amb_mode}", "CLARIFY/AMBIGUOUS_ACTIVITY",
                    "regional_or_agro_ambiguous_activity_clarify", "clarify_ambiguous_activity", [],
                    "Umbrella or regional cultural terms without a specific CNAE sector require clarification.", split, batch_id
                )
            elif sub == 4:
                # Invalid calendar date or missing historical month -> clarify
                if local_idx % 2 == 0:
                    bad_date = ["31/02/2025", "30/02/2025", "2025-13-01", "31/04/2025"][(local_idx // 2) % 4]
                    text = f"{opener}Quantos estabelecimentos de {sname} abriram {place_phrase} de {bad_date} a 31/05/2025?"
                    cap = "clarify_invalid_date_or_cnpj"
                    rat = "Impossible calendar date must fail closed with clarification."
                else:
                    text = f"{opener}Quantos estabelecimentos de {sname} estavam ativos {place_phrase} em 2025?"
                    cap = "clarify_missing_period"
                    rat = "Historical stock query for a full year without specifying a monthly competence requires clarification."
                gold = {"decision": "clarify", "intent": None, "ir": None, "expected_behavior": rat}
                return self._make_item(
                    item_num, text, register, gold, ["SR-05", "SR-10"],
                    fam, "Detect invalid calendar date or underspecified historical snapshot competence", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"prov_clarify_temporal_{local_idx % 2}", "CLARIFY/TEMPORAL_VALIDATION",
                    "temporal_validation_guard", cap, [], rat, split, batch_id
                )
            else:
                # Informal MT subregion ('no Nortão', 'na Baixada Cuiabana', 'no Vale do Araguaia') without municipalities -> clarify
                subreg = ["no Nortão de Mato Grosso", "na Baixada Cuiabana", "no Vale do Araguaia"][(unit_idx + local_idx) % 3]
                text = f"{opener}Quantos estabelecimentos ativos de {sname} existem {subreg}?"
                r_ids = list(dict.fromkeys(reg_ids + ["reg_mt_informal_subregions", "REG-MT-03"]))
                gold = {
                    "decision": "clarify",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Clarify which specific municipalities or UF should be included for the informal subregion.",
                }
                item_uf = uf if geo_stratum == "other_26_ufs" else "MT"
                return self._make_item(
                    item_num, text, register, gold, ["SR-06", "SR-09"],
                    fam, "Clarify informal regional designation not mapped to a single catalogue entity", geo_stratum, item_uf, r_ids, True, ouf_reg,
                    "prov_clarify_subregion", "CLARIFY/INFORMAL_SUBREGION",
                    "mt_informal_subregion_clarification", "clarify_homonym", [],
                    "Informal subregions ('Nortão', 'Baixada Cuiabana', 'Vale do Araguaia') require specifying the municipalities.", split, batch_id
                )

        if fam == "unsupported_missing_contradictions":
            sub = local_idx % 5
            if sub == 0:
                # Financial attributes (faturamento, lucro, folha salarial, funcionários) -> reject
                attr = [
                    "o faturamento mensal médio",
                    "o lucro líquido anual",
                    "a folha de pagamento total",
                    "a quantidade total de funcionários CLT",
                ][local_idx % 4]
                text = f"{opener}Qual é {attr} dos estabelecimentos de {sname} {place_phrase}?"
                gold = {
                    "decision": "reject",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Reject: revenue, profit, payroll, and employee headcount are absent from public RFB CNPJ data.",
                }
                return self._make_item(
                    item_num, text, register, gold, ["SR-09", "SR-10"],
                    fam, "Refuse unavailable financial or headcount attributes", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "unsup_financial_headcount", "REJECT/UNAVAILABLE_FINANCIAL",
                    None, "reject_unavailable_cadastral", [],
                    "Financial and employee headcount figures do not exist in public CNPJ data.", split, batch_id
                )
            elif sub == 1:
                # Private partner / CPF / pau-rodado origin / B2B client list -> reject
                priv_mode = local_idx % 3
                if priv_mode == 0:
                    text = f"{opener}Traga o CPF e o telefone pessoal dos sócios das empresas de {sname} {place_phrase}."
                    r_ids = reg_ids
                elif priv_mode == 1:
                    text = f"{opener}Quantas empresas de {sname} {place_phrase} pertencem a sócios pau-rodados nascidos fora do estado?"
                    r_ids = list(dict.fromkeys(reg_ids + ["reg_mt_pau_rodado", "REG-MT-03"]))
                else:
                    text = f"{opener}Quem são os principais clientes e fornecedores das empresas de {sname} {place_phrase}?"
                    r_ids = list(dict.fromkeys(reg_ids + ["biz_amb_saturation"]))
                gold = {
                    "decision": "reject",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Reject: partner CPFs, personal phones, partner birthplace/origin, and private B2B client/supplier graphs are unavailable.",
                }
                return self._make_item(
                    item_num, text, register, gold, ["SR-09", "SR-10", "SR-12"],
                    fam, "Refuse private partner data, partner origin, or B2B transaction graphs", geo_stratum, uf, r_ids, True if priv_mode == 1 else mt_ctx, ouf_reg,
                    f"unsup_privacy_or_b2b_{priv_mode}", "REJECT/PRIVACY_OR_CLIENTS",
                    "partner_origin_pau_rodado_or_b2b_graph_refusal", "reject_privacy_or_advice", [],
                    "Personal partner attributes and B2B client/supplier lists are outside public cadastral scope.", split, batch_id
                )
            elif sub == 2:
                # Future forecast (2027 / 2028) or subjective investment advice -> reject
                if local_idx % 2 == 0:
                    fut_year = 2027 if (local_idx % 4 == 0) else 2028
                    text = f"{opener}Quantos estabelecimentos de {sname} vão abrir em {fut_year} {place_phrase}?"
                    beh = f"Reject: cannot forecast business registrations for future year {fut_year}."
                else:
                    text = f"{opener}Onde vale mais a pena investir para ter retorno rápido com {sname} {place_phrase}?"
                    beh = "Reject: investment return advice is outside factual cadastral search."
                gold = {"decision": "reject", "intent": None, "ir": None, "expected_behavior": beh}
                return self._make_item(
                    item_num, text, register, gold, ["SR-05", "SR-09", "SR-10"],
                    fam, "Refuse future forecasts and subjective commercial investment advice", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"unsup_forecast_advice_{local_idx % 2}", "REJECT/FORECAST_OR_ADVICE",
                    "future_forecast_or_investment_advice_refusal", "reject_statistical_correlation_or_forecast", [],
                    beh, split, batch_id
                )
            elif sub == 3:
                # Sub-municipal filter (bairro, CEP, logradouro) -> clarify
                sub_loc = ["no bairro Centro", "no CEP 78000-000", "no logradouro principal"][local_idx % 3]
                text = f"{opener}Quantos estabelecimentos ativos de {sname} existem {sub_loc} {place_phrase}?"
                gold = {
                    "decision": "clarify",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Clarify: neighborhood (bairro), CEP, and street (logradouro) filters are not supported; offer municipality-level search.",
                }
                return self._make_item(
                    item_num, text, register, gold, ["SR-06", "SR-09"],
                    fam, "Fail closed on unsupported sub-municipal street/neighborhood/CEP filter", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "unsup_submunicipal_clarify", "CLARIFY/SUBMUNICIPAL_FILTER",
                    "submunicipal_bairro_cep_logradouro_guard", "clarify_ambiguous_quality", [],
                    "Sub-municipal filters (bairro, CEP, logradouro) fail closed with clarification.", split, batch_id
                )
            else:
                # Closure with opening date filter but missing closure period -> clarify (Round 08 lesson!)
                text = f"{opener}Quantos estabelecimentos de {sname} tiveram baixa cadastral com início de atividade em 2025 {place_phrase}?"
                gold = {
                    "decision": "clarify",
                    "intent": None,
                    "ir": None,
                    "expected_behavior": "Clarify the closure period: opening date ('início de atividade em 2025') is a cohort filter, not the closure date.",
                }
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-05", "SR-10"],
                    fam, "Require explicit closure period when only opening cohort period is stated", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "unsup_closure_missing_period", "CLARIFY/CLOSURE_MISSING_EVENT_PERIOD",
                    "closure_with_opened_cohort_missing_closure_period", "clarify_missing_period", [],
                    "Opening period is a cohort filter (scope.opened) and cannot substitute for the missing closure period.", split, batch_id
                )

        if fam == "tax_size_legal_capital_numeric":
            sub = local_idx % 4
            if sub == 0:
                # Simples + non-MEI + capital threshold
                cap_reais = [100000, 200000, 300000, 500000][local_idx % 4]
                text = f"{opener}Quantos estabelecimentos ativos de {sname} {place_phrase} são optantes pelo Simples Nacional, não são MEI e possuem capital social acima de {cap_reais} reais?"
                extra = {
                    "simples": "yes",
                    "mei": "no",
                    "capital": {"from": cap_reais * 100 + 1, "to": None},
                }
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06", "SR-07"],
                    fam, "Segment establishments by Simples regime, non-MEI status, and minimum share capital", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "tax_simples_nonmei_cap", "COUNT/STOCK/SIMPLES_NONMEI_CAPITAL",
                    "triple_filter_simples_nonmei_capital", "supported_engine", [],
                    "Three-filter count combining Simples='yes', MEI='no', and capital social lower bound.", split, batch_id
                )
            elif sub == 1:
                # Porte demás (05) + Sociedade Anônima Fechada (2054) or LTDA (2062)
                use_sa = (local_idx % 2 == 1)
                nat_code = 2054 if use_sa else 2062
                nat_txt = "sociedade anônima fechada" if use_sa else "sociedade empresária limitada"
                text = f"{opener}Quantos estabelecimentos ativos de {sname} {place_phrase} possuem porte cadastral demais e natureza jurídica de {nat_txt}?"
                extra = {"porte": ["other"], "natureza": [{"code": nat_code}]}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06"],
                    fam, "Filter mid/large establishments (porte demais) by specific legal nature", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"tax_porte_other_nat_{nat_code}", "COUNT/STOCK/PORTE_OTHER_NATUREZA",
                    "porte_other_plus_legal_nature", "supported_engine", [],
                    "Count combining porte='other' (05) with explicit legal nature code.", split, batch_id
                )
            elif sub == 2:
                # Capital social range (entre X e Y reais) + head_office
                low_r = 50000
                high_r = [200000, 500000, 1000000][local_idx % 3]
                text = f"{opener}Quantos estabelecimentos ativos de {sname} {place_phrase} que são matriz possuem capital social entre {low_r} reais e {high_r} reais?"
                extra = {
                    "head_office": "yes",
                    "capital": {"from": low_r * 100, "to": high_r * 100},
                }
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-02", "SR-03", "SR-06"],
                    fam, "Filter head offices within bounded share capital bracket", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "tax_hq_capital_between", "COUNT/STOCK/HQ_CAPITAL_BETWEEN",
                    "head_office_plus_bounded_capital_interval", "supported_engine", [],
                    "Count of active head offices with bounded share capital interval in cents.", split, batch_id
                )
            else:
                # MEI opt-ins or exits flow in 2025
                is_exit = (local_idx % 2 == 1)
                flow_code = "mei_exits" if is_exit else "mei_opt_ins"
                flow_txt = "exclusões do MEI" if is_exit else "opções pelo MEI"
                text = f"{opener}Quantos estabelecimentos de {sname} tiveram {flow_txt} em 2025 {place_phrase}?"
                period = {"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                ir = base_ir(
                    {"kind": "count", "metric": {"flow": flow_code}},
                    place_ir, activity_sector(s), status=ALL_STATUS, period=period
                )
                gold = {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"}
                return self._make_item(
                    item_num, text, register, gold, ["SR-04", "SR-05", "SR-06"],
                    fam, "Track MEI tax regime entries or exits over calendar year", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"tax_flow_{flow_code}", f"COUNT/{flow_code.upper()}/YEAR",
                    None, "supported_engine", [],
                    f"Flow count of {flow_code} in 2025.", split, batch_id
                )

        raise ValueError(f"Unknown family: {fam}")

    def _gen_dialogue(
        self,
        unit_idx: int,
        d_idx: int,
        split: str,
        geos: list[str],
        regs: list[str],
        session_id: str,
        batch_id: str,
        start_item_num: int,
    ) -> list[dict]:
        fam = "dialogue_corrections_references_planning"
        seed = unit_idx * 503 + d_idx * 19 + start_item_num
        uf1, place1, phrase1, mt1, ouf1, reg1, opener1, m1 = self._resolve_geo(geos[0], seed)
        if m1 is None:
            m1 = self.next_mt_muni("mt_north_agro_industrial", seed)
            uf1, place1, phrase1, mt1 = "MT", place_muni(m1), f"na cidade de {m1['name']}/MT", True

        uf2, place2, phrase2, mt2, ouf2, reg2, _, m2 = self._resolve_geo(geos[1], seed + 7)
        if m2 is None or m2["id"] == m1["id"]:
            m2 = self.next_mt_muni("mt_south_logistics_industrial", seed + 11)
            uf2, place2, phrase2, mt2 = "MT", place_muni(m2), f"na cidade de {m2['name']}/MT", True

        s1 = self.pick_sector(seed)
        s2 = self.pick_sector(seed + 5)
        if s2["id"] == s1["id"]:
            s2 = self.pick_sector(seed + 9)

        pattern = d_idx % 4
        turns = []

        if pattern == 0:
            # Turn 1: Base count -> Turn 2: add/refine filter (head_office + porte) -> Turn 3: correction to city 2
            t1_txt = f"{opener1}Quantos estabelecimentos ativos de {s1['sing']} existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(self._make_item(
                start_item_num, t1_txt, regs[0],
                {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], fam, "Start territory sizing dialogue", geos[0], m1["uf"], reg1, mt1, ouf1,
                "dlg_p0_t1", "DLG/P0/T1_NEW", None, "supported_aggregates", [],
                "Dialogue turn 1: initial establishment count.", split, batch_id,
                context={"session_id": session_id, "turn": 1}
            ))

            if unit_idx == 0 and d_idx < 4:
                t2_txt = "Agora considere somente matrizes de microempresa."
                t2_act = "add_filter"
            else:
                t2_txt = f"Agora considere somente matrizes de microempresa de {s1['sing']} na cidade de {m1['name']}/{m1['uf']}."
                t2_act = "replace_slot"
            ir2 = base_ir(
                {"kind": "count", "metric": "stock"},
                place_muni(m1), activity_sector(s1),
                extra_scope={"head_office": "yes", "porte": ["micro"]}
            )
            turns.append(self._make_item(
                start_item_num + 1, t2_txt, regs[1],
                {"decision": "execute", "intent": "count", "ir": ir2, "act": t2_act},
                ["SR-02", "SR-03", "SR-06"], fam, "Refine dialogue context with head-office and micro-enterprise filters", geos[1], m1["uf"], [], mt1, False,
                "dlg_p0_t2", "DLG/P0/T2_ADD_FILTER", "dialogue_compound_filter_refinement", "supported_aggregates", [],
                "Dialogue turn 2: add head_office='yes' and porte=['micro'] preserving place and sector.", split, batch_id,
                context={"session_id": session_id, "turn": 2}
            ))

            t3_txt = f"Na verdade, quis dizer para estabelecimentos de {s1['sing']} na cidade de {m2['name']}/{m2['uf']}."
            ir3 = base_ir(
                {"kind": "count", "metric": "stock"},
                place_muni(m2), activity_sector(s1),
                extra_scope={"head_office": "yes", "porte": ["micro"]}
            )
            turns.append(self._make_item(
                start_item_num + 2, t3_txt, regs[2],
                {"decision": "execute", "intent": "count", "ir": ir3, "act": "correction"},
                ["SR-02", "SR-06", "SR-07"], fam, "Correct target municipality while preserving compound filters", geos[2], m2["uf"], reg2, mt2, ouf2,
                "dlg_p0_t3", "DLG/P0/T3_CORRECTION", "dialogue_city_correction_preserving_filters", "supported_aggregates", [],
                "Dialogue turn 3: replace municipality slot via correction while keeping active filters.", split, batch_id,
                context={"session_id": session_id, "turn": 3}
            ))

        elif pattern == 1:
            # Turn 1: Count with head_office='yes' -> Turn 2: remove_filter -> Turn 3: replace_slot to new sector
            t1_txt = f"{opener1}Quantos estabelecimentos ativos de {s1['sing']} que são matriz existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1), extra_scope={"head_office": "yes"})
            turns.append(self._make_item(
                start_item_num, t1_txt, regs[0],
                {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], fam, "Start head-office filtered query in dialogue", geos[0], m1["uf"], reg1, mt1, ouf1,
                "dlg_p1_t1", "DLG/P1/T1_NEW_HQ", None, "supported_aggregates", [],
                "Dialogue turn 1: count active head-office establishments.", split, batch_id,
                context={"session_id": session_id, "turn": 1}
            ))

            if unit_idx == 0 and d_idx < 4:
                t2_txt = "Agora retire o filtro de matriz e conte matrizes e filiais."
            else:
                t2_txt = f"Agora retire o filtro de matriz e conte matrizes e filiais de {s1['sing']} na cidade de {m1['name']}/{m1['uf']}."
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(self._make_item(
                start_item_num + 1, t2_txt, regs[1],
                {"decision": "execute", "intent": "count", "ir": ir2, "act": "remove_filter"},
                ["SR-02", "SR-03"], fam, "Remove head-office restriction in follow-up turn", geos[1], m1["uf"], [], mt1, False,
                "dlg_p1_t2", "DLG/P1/T2_REMOVE_FILTER", None, "supported_aggregates", [],
                "Dialogue turn 2: clear head_office restriction.", split, batch_id,
                context={"session_id": session_id, "turn": 2}
            ))

            t3_txt = f"E para estabelecimentos de {s2['sing']} na cidade de {m1['name']}/{m1['uf']}?"
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s2))
            turns.append(self._make_item(
                start_item_num + 2, t3_txt, regs[2],
                {"decision": "execute", "intent": "count", "ir": ir3, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-07"], fam, "Switch target sector in same municipality", geos[2], m1["uf"], [], mt1, False,
                "dlg_p1_t3", "DLG/P1/T3_REPLACE_SECTOR", "dialogue_sector_pivot", "supported_aggregates", [],
                "Dialogue turn 3: replace sector while keeping municipality.", split, batch_id,
                context={"session_id": session_id, "turn": 3}
            ))

        elif pattern == 2:
            # Turn 1: Compare two cities on stock -> Turn 2: follow-up compare on focus -> Turn 3: single-city count
            members = [{"municipality": mid} for mid in sorted({m1["id"], m2["id"]})]
            t1_txt = f"{opener1}Compare a quantidade de estabelecimentos ativos de {s1['sing']} entre a cidade de {m1['name']}/{m1['uf']} e a cidade de {m2['name']}/{m2['uf']}."
            ir1 = base_ir({"kind": "compare", "dim": "municipality", "members": members, "metric": "stock"}, "any", activity_sector(s1))
            turns.append(self._make_item(
                start_item_num, t1_txt, regs[0],
                {"decision": "execute", "intent": "compare", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], fam, "Compare two municipalities in dialogue", geos[0], m1["uf"], reg1, mt1, ouf1,
                "dlg_p2_t1", "DLG/P2/T1_COMPARE", None, "supported_aggregates", [],
                "Dialogue turn 1: two-municipality stock comparison.", split, batch_id,
                context={"session_id": session_id, "turn": 1}
            ))

            t2_txt = f"E qual das duas tem mais estabelecimentos ativos de {s1['sing']} por 10 mil habitantes?"
            ir2 = base_ir(
                {"kind": "compare", "dim": "municipality", "members": members, "metric": {"per10k": {"ibge_estimate": 2026}}},
                "any", activity_sector(s1)
            )
            turns.append(self._make_item(
                start_item_num + 1, t2_txt, regs[1],
                {"decision": "execute", "intent": "compare", "ir": ir2, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-11"], fam, "Switch comparison metric to per-10k density using dialogue focus", geos[1], m1["uf"], [], mt1, False,
                "dlg_p2_t2", "DLG/P2/T2_FOCUS_PER10K", "dialogue_focus_metric_switch", "supported_aggregates", [],
                "Dialogue turn 2: compare focus members on per-10k density.", split, batch_id,
                context={"session_id": session_id, "turn": 2}
            ))

            t3_txt = f"Quantos estabelecimentos ativos de {s1['sing']} existem na cidade de {m2['name']}/{m2['uf']} na consulta?"
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m2), activity_sector(s1))
            turns.append(self._make_item(
                start_item_num + 2, t3_txt, regs[2],
                {"decision": "execute", "intent": "count", "ir": ir3, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], fam, "Pivot from comparison to single-city stock count", geos[2], m2["uf"], reg2, mt2, ouf2,
                "dlg_p2_t3", "DLG/P2/T3_SINGLE_COUNT", None, "supported_aggregates", [],
                "Dialogue turn 3: explicit single-municipality count.", split, batch_id,
                context={"session_id": session_id, "turn": 3}
            ))

        else:
            # Pattern 3: Turn 1 homonym clarify -> Turn 2 explicit UF resolution -> Turn 3 add_filter MEI
            mt_homonyms = [
                ("Água Boa", 5170),
                ("Canarana", 5171),
                ("Várzea Grande", 5158),
                ("Pedra Preta", 5165),
                ("Rio Branco", 5072),
                ("Santa Terezinha", 5173),
                ("Nova Olímpia", 5520),
                ("General Carneiro", 5113),
                ("Novo Santo Antônio", 1088),
            ]
            h_name, h_id = mt_homonyms[(unit_idx + d_idx) % len(mt_homonyms)]
            h_muni = self.munis[h_id]

            t1_txt = f"Quantos estabelecimentos ativos de {s1['sing']} existem na cidade de {h_name}?"
            turns.append(self._make_item(
                start_item_num, t1_txt, regs[0],
                {"decision": "clarify", "intent": None, "ir": None, "expected_behavior": f"Ask which UF is meant for {h_name}."},
                ["SR-08"], fam, "Trigger homonym clarification in turn 1 of dialogue", geos[0], "UNRESOLVED", ["REG-MT-03"], True, False,
                "dlg_p3_t1_clarify", "DLG/P3/T1_HOMONYM_CLARIFY", "dialogue_homonym_clarify_then_resolve", "clarify_homonym", [],
                "Dialogue turn 1: homonymous municipality without UF triggers clarification.", split, batch_id,
                context={"session_id": session_id, "turn": 1}
            ))

            t2_txt = f"Quantos estabelecimentos ativos de {s1['sing']} existem na cidade de {h_name}/MT?"
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(h_muni), activity_sector(s1))
            turns.append(self._make_item(
                start_item_num + 1, t2_txt, regs[1],
                {"decision": "execute", "intent": "count", "ir": ir2, "act": "new_query"},
                ["SR-02", "SR-06", "SR-08"], fam, "Resolve homonym clarification by supplying MT UF", geos[1], "MT", ["REG-MT-03"], True, False,
                "dlg_p3_t2_resolve", "DLG/P3/T2_RESOLVE_UF", "dialogue_homonym_clarify_then_resolve", "supported_aggregates", [],
                "Dialogue turn 2: user supplies MT UF to resolve homonymous municipality.", split, batch_id,
                context={"session_id": session_id, "turn": 2}
            ))

            if unit_idx == 0 and d_idx < 4:
                t3_txt = "Agora filtre somente optantes pelo MEI."
                t3_act = "add_filter"
            else:
                t3_txt = f"Agora filtre somente optantes pelo MEI de {s1['sing']} na cidade de {h_name}/MT."
                t3_act = "replace_slot"
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(h_muni), activity_sector(s1), extra_scope={"mei": "yes"})
            turns.append(self._make_item(
                start_item_num + 2, t3_txt, regs[2],
                {"decision": "execute", "intent": "count", "ir": ir3, "act": t3_act},
                ["SR-02", "SR-03", "SR-06"], fam, "Add MEI filter to resolved dialogue state", geos[2], "MT", ["REG-MT-03"], True, False,
                "dlg_p3_t3_add_mei", "DLG/P3/T3_ADD_MEI", "dialogue_homonym_clarify_then_resolve", "supported_aggregates", [],
                "Dialogue turn 3: add MEI='yes' filter on top of resolved turn 2 state.", split, batch_id,
                context={"session_id": session_id, "turn": 3}
            ))

        return turns


def write_batch_files(batch_id: str, items: list[dict]):
    BATCHES_DIR.mkdir(parents=True, exist_ok=True)
    bpath = BATCHES_DIR / f"{batch_id}.jsonl"
    lines = [json.dumps(item, ensure_ascii=False) for item in items]
    content = ("\n".join(lines) + "\n").encode("utf-8")
    bpath.write_bytes(content)

    fams = collections.Counter(x["meta"]["primary_family"] for x in items)
    geos = collections.Counter(x["meta"]["geographic_stratum"] for x in items)
    regs = collections.Counter(x["meta"]["register"] for x in items)
    splits = collections.Counter(x["meta"]["split"] for x in items)
    decs = collections.Counter(x["gold"]["decision"] for x in items)

    bman = {
        "batch_id": batch_id,
        "items": len(items),
        "first_id": items[0]["id"],
        "last_id": items[-1]["id"],
        "sha256": sha256_bytes(content),
        "by_family": dict(fams),
        "by_geographic_stratum": dict(geos),
        "by_register": dict(regs),
        "by_split": dict(splits),
        "by_decision": dict(decs),
    }
    (BATCHES_DIR / f"{batch_id}-manifest.json").write_text(json.dumps(bman, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return bman


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pilot-only", action="store_true", help="Generate only batch-001 (1,000-item pilot)")
    parser.add_argument("--all", action="store_true", help="Generate all 99 batches (50,000 items) and merged files")
    args = parser.parse_args()

    author = Round09Author()

    # Batch 001 is the 1,000-item pilot (units 0 and 1)
    u0 = author.build_unit(0, "batch-001", 1, 1)
    u1 = author.build_unit(1, "batch-001", 501, 21)
    pilot_items = u0 + u1
    assert len(pilot_items) == 1000
    pilot_man = write_batch_files("batch-001", pilot_items)
    print(f"Wrote batch-001 (pilot): {len(pilot_items)} items, sha256={pilot_man['sha256']}")

    if args.pilot_only and not args.all:
        return

    all_items = list(pilot_items)
    batch_manifests = [pilot_man]
    for unit_idx in range(2, 100):
        batch_id = f"batch-{unit_idx:03d}"
        start_item = unit_idx * 500 + 1
        start_sess = unit_idx * 20 + 1
        b_items = author.build_unit(unit_idx, batch_id, start_item, start_sess)
        bman = write_batch_files(batch_id, b_items)
        batch_manifests.append(bman)
        all_items.extend(b_items)
        if unit_idx % 20 == 0 or unit_idx == 99:
            print(f"Generated through {batch_id}: total items = {len(all_items)}")

    assert len(all_items) == 50000

    # Write merged questions.jsonl and questions.tsv
    q_jsonl = ("\n".join(json.dumps(x, ensure_ascii=False) for x in all_items) + "\n").encode("utf-8")
    q_path = EVAL_DIR / "questions.jsonl"
    q_path.write_bytes(q_jsonl)

    tsv_lines = ["id\tsplit\tprimary_family\tgeographic_stratum\tuf\tregister\tdecision\ttext"]
    for x in all_items:
        m = x["meta"]
        tsv_lines.append(
            f"{x['id']}\t{m['split']}\t{m['primary_family']}\t{m['geographic_stratum']}\t{m['uf']}\t{m['register']}\t{x['gold']['decision']}\t{x['text']}"
        )
    tsv_bytes = ("\n".join(tsv_lines) + "\n").encode("utf-8")
    tsv_path = EVAL_DIR / "questions.tsv"
    tsv_path.write_bytes(tsv_bytes)

    # Also write split-specific JSONL files for independent split auditing
    for sp in ("synthetic_dev", "synthetic_eval", "synthetic_challenge"):
        sp_items = [x for x in all_items if x["meta"]["split"] == sp]
        sp_bytes = ("\n".join(json.dumps(x, ensure_ascii=False) for x in sp_items) + "\n").encode("utf-8")
        (EVAL_DIR / f"{sp}.jsonl").write_bytes(sp_bytes)

    fams = collections.Counter(x["meta"]["primary_family"] for x in all_items)
    geos = collections.Counter(x["meta"]["geographic_stratum"] for x in all_items)
    regs = collections.Counter(x["meta"]["register"] for x in all_items)
    splits = collections.Counter(x["meta"]["split"] for x in all_items)
    decs = collections.Counter(x["gold"]["decision"] for x in all_items)
    mt_ctx_cnt = sum(1 for x in all_items if x["meta"]["mt_context"])
    ouf_reg_cnt = sum(1 for x in all_items if x["meta"]["other_uf_regional"])
    novel_cnt = sum(1 for x in all_items if x["meta"]["novelty_reason"] is not None)

    manifest = {
        "round": "commercial-round-09",
        "items": len(all_items),
        "batches": len(batch_manifests),
        "pilot_batch": "batch-001",
        "pilot_items": 1000,
        "questions_jsonl_sha256": sha256_bytes(q_jsonl),
        "questions_tsv_sha256": sha256_bytes(tsv_bytes),
        "data_anchor": "2026-10-11",
        "current_competence": "2026-10",
        "catalogue_versions": VERSIONS,
        "by_primary_family": dict(fams),
        "by_geographic_stratum": dict(geos),
        "by_register": dict(regs),
        "by_split": dict(splits),
        "by_decision": dict(decs),
        "overlapping_counts": {
            "mt_context_items": mt_ctx_cnt,
            "other_uf_regional_items": ouf_reg_cnt,
            "novel_semantic_constructions": novel_cnt,
            "clarify_or_reject_items": decs["clarify"] + decs["reject"],
        },
        "human_validated": False,
    }
    (EVAL_DIR / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (EVAL_DIR / "batch-manifests.json").write_text(json.dumps(batch_manifests, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    card = f"""# Dataset Card — Commercial Round 09 (`commercial-round-09`)

## Overview

- **Total items**: `50,000` pt-BR commercial questions and dialogue turns (`batch-001` 1,000-item pilot + `batch-002..batch-099` 98 batches of 500 items).
- **Questions JSONL SHA-256**: `{manifest['questions_jsonl_sha256']}`
- **Questions TSV SHA-256**: `{manifest['questions_tsv_sha256']}`
- **Data anchor**: `2026-10-11` (`Day(46305)`), current competence `2026-10` (`Competence(321)`).
- **Human validation**: `false` (deferred to product refinement per `ROUND-09-EXPANSION.md`).

## Quotas & Distributions

- **Geography (`50,000`)**:
  - Mato Grosso (`30,000` across 6 strata covering all `142/142` MT municipalities): `mt_metropolitan` (`5,000`), `mt_north_agro_industrial` (`6,000`), `mt_south_logistics_industrial` (`5,000`), `mt_east_araguaia_xingu` (`4,500`), `mt_west_parecis_guapore` (`4,500`), `mt_pantanal` (`5,000`).
  - Other 26 UFs (`12,000`, minimum `>= 400` per UF).
  - National, cross-UF, macro-regional, comparative, and unresolved-homonym (`8,000`).
- **Primary Business Families (`50,000`)**:
  - `market_size_density_competition`: `5,000`
  - `prospecting_qualification_territory`: `7,000`
  - `historical_stock_flows_seasonality`: `5,000`
  - `geography_catchments_comparisons`: `5,000`
  - `corporate_structure_hq_branches_identity`: `4,000`
  - `supply_chain_complementary`: `5,000`
  - `cross_variable_correlation`: `5,000`
  - `dialogue_corrections_references_planning`: `6,000` (`2,000` complete 3-turn dialogues)
  - `source_quality_provenance_uncertainty`: `3,000`
  - `unsupported_missing_contradictions`: `3,000`
  - `tax_size_legal_capital_numeric`: `2,000`
- **Registers (`50,000`)**: `conversational_professional` (`35,000`), `formal_technical` (`10,000`), `noisy_short` (`5,000`).
- **Splits (`50,000`)**: `synthetic_dev` (`35,000`), `synthetic_eval` (`10,000`), `synthetic_challenge` (`5,000`), strictly partitioned by `scenario_family_id`, `surface_skeleton_id`, CNPJ lookup roots, and `session_id`.
- **Overlapping Targets**:
  - MT-context regional/economic items: `{mt_ctx_cnt}` (`>= 8,000` target)
  - Other-UF regional items: `{ouf_reg_cnt}` (`>= 4,000` target)
  - Novel semantic constructions vs rounds 01–08: `{novel_cnt}` (`>= 15,000` target)
  - `clarify` + `reject` outcomes: `{decs['clarify'] + decs['reject']}` (`{decs['clarify']}` `clarify` + `{decs['reject']}` `reject`; `>= 5,000` target)
"""
    (EVAL_DIR / "DATASET-CARD.md").write_text(card, encoding="utf-8")
    print("Completed full 50,000-item generation and manifests.")


if __name__ == "__main__":
    main()
