#!/usr/bin/env python3
"""Author the 50,000-item Commercial Round 13 evaluation set.

Reuses the Round 09 authoring machinery (catalogue loading, quota schedules,
register transforms, uniqueness guard) but replaces every scenario surface with
Round 13 constructions aimed at the documented Round 09 gaps:

* sector colloquialisms mapped to curated catalogue sectors;
* compound multi-turn scope edits (keep/drop/switch);
* exact-name versus prefix-sharing municipality disambiguation;
* numeric boundaries, coordinated exclusions and alternative comparisons.

All items stay synthetic. No customer, interview, quote or correlation
coefficient is invented.
"""

from __future__ import annotations

import argparse
import collections
import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
EVAL_DIR = ROOT / "crates/opencnpj-lab/eval/commercial-round-13"
BATCHES_DIR = EVAL_DIR / "batches"
REFERENCE = EVAL_DIR / "round09_reference.py"

spec = importlib.util.spec_from_file_location("r09ref", REFERENCE)
r09 = importlib.util.module_from_spec(spec)
assert spec.loader is not None
spec.loader.exec_module(r09)

ROUND = "commercial-round-13"
ID_PREFIX = "commercial-r13"
ANCHOR_DAY = r09.ANCHOR_DAY
CURRENT_COMP = r09.CURRENT_COMP
VERSIONS = r09.VERSIONS
ALL_STATUS = r09.ALL_STATUS
UF_ID = r09.UF_ID
UF_NAME = r09.UF_NAME
NON_MT_UFS = r09.NON_MT_UFS
day_from_ymd = r09.day_from_ymd
comp_from_ym = r09.comp_from_ym
sha256_bytes = r09.sha256_bytes
base_ir = r09.base_ir
place_muni = r09.place_muni
place_uf = r09.place_uf
activity_sector = r09.activity_sector
activity_two_sectors = r09.activity_two_sectors
fold = r09.fold

MT_OPENERS = [
    ("Vôte, ", ["reg_mt_vote", "REG-MT-01", "REG-MT-03"]),
    ("Tchá por Deus, ", ["reg_mt_tcha_por_deus", "REG-MT-01", "REG-MT-02"]),
    ("No muxirum da equipe comercial: ", ["reg_mt_muxirum", "REG-MT-03"]),
    ("Como pau-rodado recém-chegado no estado, pergunto: ", ["reg_mt_pau_rodado", "REG-MT-03"]),
    ("Olhando o corredor mato-grossense: ", ["REG-MT-03"]),
    ("Pra atender a peãozada depois do quebra-torto: ", ["reg_mt_quebra_torto", "REG-MT-03"]),
]

UF_OPENERS = r09.UF_DISCOURSE_OPENERS

# Documented commercial surfaces already used in the catalogue; each pair maps
# to the same treasury sector id as its display form.
COLLOQUIAL = {
    "auto_mechanic": [("oficina", "oficinas"), ("mecânica", "mecânicas"), ("retífica", "retíficas"), ("autocenter", "autocenters"), ("autocentro", "autocentros"), ("troca de óleo", "trocas de óleo")],
    "fuel_station": [("posto", "postos")],
    "body_shop": [("funilaria", "funilarias")],
    "restaurant": [("pizzaria", "pizzarias"), ("churrascaria", "churrascarias")],
    "pet_shop": [("pet", "pets")],
    "advertising_printing": [("gráfica", "gráficas")],
    "electrical_installation": [("eletricista", "eletricistas")],
    "meat_processing": [("frigorífico", "frigoríficos"), ("abatedouro", "abatedouros")],
    "gym": [("academia", "academias")],
    "snack_bar": [("hamburgueria", "hamburguerias"), ("cafeteria", "cafeterias"), ("sorveteria", "sorveterias"), ("pastelaria", "pastelarias")],
    "beverage_retail": [("adega", "adegas")],
    "tire_repair": [("vulcanizadora", "vulcanizadoras")],
    "supermarket": [("atacarejo", "atacarejos")],
    "tobacco_shop": [("charutaria", "charutarias")],
    "variety_store": [("lojinha", "lojinhas")],
    "candy_store": [("doceria", "docerias")],
    "bar": [("choperia", "choperias")],
    "grocery": [("empório", "empórios")],
}

CONTACT = [
    ("phone", "com telefone cadastrado", "com telefone"),
    ("email", "com e-mail cadastrado", "com e-mail"),
    ("both", "com telefone e e-mail cadastrados", "com telefone e e-mail"),
]


def cap(text: str) -> str:
    return text[:1].upper() + text[1:]


class Round13Author(r09.Round09Author):
    def __init__(self) -> None:
        super().__init__()
        import json as _json
        for prior_name in ("commercial-round-10", "commercial-round-11", "commercial-round-12"):
            prior_round = Path(__file__).resolve().parent.parent / prior_name / "questions.jsonl"
            if prior_round.exists():
                with prior_round.open(encoding="utf-8") as f:
                    for line in f:
                        if line.strip():
                            row = _json.loads(line)
                            if "text" in row:
                                self.existing_folded.add(fold(row["text"]))
        self._geo_prefix = False
        self._colloquial_seen = 0
        by_folded = collections.defaultdict(list)
        for m in self.munis.values():
            by_folded[m["folded"]].append(m)
        self.prefix_folded = {
            name
            for name, rows in by_folded.items()
            if len(rows) == 1 and any(other != name and other.startswith(name + " ") for other in by_folded)
        }
        self.prefix_by_uf = collections.defaultdict(list)
        for m in sorted(self.munis.values(), key=lambda x: (-x["pop_2026"], x["id"])):
            if m["folded"] in self.prefix_folded:
                self.prefix_by_uf[m["uf"]].append(m)
        excluded = {fold(n) for n in ["Acre", "Alagoas", "Amazonas", "Amapá", "Bahia", "Ceará", "Distrito Federal", "Espírito Santo", "Goiás", "Maranhão", "Minas Gerais", "Mato Grosso do Sul", "Mato Grosso", "Pará", "Paraíba", "Pernambuco", "Piauí", "Paraná", "Rio de Janeiro", "Rio Grande do Norte", "Rondônia", "Roraima", "Rio Grande do Sul", "Santa Catarina", "Sergipe", "São Paulo", "Tocantins", "Norte", "Nordeste", "Sudeste", "Sul", "Centro-Oeste"]}
        self.ambiguous_geo_names = excluded
        self.sector_surface_folded = set()
        for sec in self.sectors:
            self.sector_surface_folded.add(fold(sec["sing"]))
            self.sector_surface_folded.add(fold(sec["plural"]))
        self.prefix_pool = [m for m in sorted(self.munis.values(), key=lambda x: x["id"]) if m["folded"] in self.prefix_folded and m["folded"] not in excluded]
        self._geo_prefix = False
        self._colloquial_seen = 0
        # Cruzeiro/SP is the only exact canonical "Cruzeiro" in the catalogue;
        # it must execute rather than be treated as a homonym (Round 09 lesson).
        self.homonym_names = [h for h in self.homonym_names if h != "Cruzeiro"]

    def build_unit(self, unit_idx, batch_id, start_item_num, start_sess_num):
        items = super().build_unit(unit_idx, batch_id, start_item_num, start_sess_num)
        for item in items:
            ctx = item.get("context")
            if ctx and isinstance(ctx.get("session_id"), str):
                ctx["session_id"] = ctx["session_id"].replace("r09-sess-", "r13-sess-")
        return items

    # -- geography -------------------------------------------------------
    def next_other_uf_muni(self):
        uf = self.uf_schedule[self.uf_cursor % 12000]
        self.uf_cursor += 1
        lst = self.uf_munis[uf]
        for _ in range(len(lst)):
            idx = self.muni_cursors[f"uf_{uf}"]
            self.muni_cursors[f"uf_{uf}"] += 1
            pool = self.prefix_by_uf.get(uf, [])
            m = pool[(idx // 2) % len(pool)] if pool and idx % 2 == 0 else lst[idx % len(lst)]
            if m["folded"] not in self.ambiguous_geo_names:
                return uf, m
        return uf, lst[0]

    def _resolve_geo(self, geo_stratum: str, seed: int):
        self._geo_prefix = False
        if geo_stratum.startswith("mt_"):
            m = self.next_mt_muni(geo_stratum, seed)
            if m["folded"] in self.ambiguous_geo_names:
                m = self.next_mt_muni(geo_stratum, seed + 1)
            use_opener = seed % 2 == 0
            opener, reg_ids = MT_OPENERS[seed % len(MT_OPENERS)] if use_opener else ("", ["REG-MT-03"])
            style = seed % 3
            bare = m["folded"] in self.prefix_folded and seed % 2 == 0
            if bare:
                p_phrase = f"na cidade de {m['name']}"
                self._geo_prefix = True
            elif style == 0:
                p_phrase = f"na cidade de {m['name']}/MT"
            elif style == 1:
                p_phrase = f"no município de {m['name']}/MT"
            else:
                p_phrase = f"em {m['name']}/MT"
            return "MT", place_muni(m), p_phrase, True, False, reg_ids, opener, m
        if geo_stratum == "other_26_ufs":
            uf, m = self.next_other_uf_muni()
            use_reg = seed % 2 == 0 and uf in UF_OPENERS
            opener, reg_ids = UF_OPENERS[uf] if use_reg else ("", [])
            style = seed % 3
            bare = m["folded"] in self.prefix_folded and seed % 2 == 0
            if bare:
                p_phrase = f"na cidade de {m['name']}"
                self._geo_prefix = True
            elif style == 0:
                p_phrase = f"na cidade de {m['name']}/{uf}"
            elif style == 1:
                p_phrase = f"no município de {m['name']}/{uf}"
            else:
                p_phrase = f"em {m['name']}/{uf}"
            return uf, place_muni(m), p_phrase, False, bool(reg_ids), reg_ids, opener, m
        sub = seed % 4
        if sub == 0:
            return "BR", "any", "no Brasil", False, False, [], "", None
        if sub == 1:
            regions = [
                (1, "na região Norte"),
                (2, "na região Nordeste"),
                (3, "na região Sudeste"),
                (4, "na região Sul"),
                (5, "na região Centro-Oeste"),
            ]
            rid, rphrase = regions[seed % 5]
            return "BR", {"region": rid}, rphrase, rid == 5, False, ["REG-MT-03"] if rid == 5 else [], "", None
        if sub == 2:
            return "MT", place_uf("MT"), "no estado de Mato Grosso", True, False, ["REG-MT-03"], "", None
        uf = NON_MT_UFS[seed % len(NON_MT_UFS)]
        return uf, place_uf(uf), f"em {uf}", False, False, [], "", None

    # -- surfaces --------------------------------------------------------
    def sector_surface(self, s, seed, force=False):
        opts = COLLOQUIAL.get(s["key"], [])
        if opts and (force or seed % 5 != 0):
            sing, plural = opts[seed % len(opts)]
            self._colloquial_seen += 1
            return sing, plural, True
        return s["sing"], s["plural"], False

    def frames(self, split: str, dev: str, ev: str, ch: str) -> str:
        if split == "synthetic_dev":
            return dev
        if split == "synthetic_eval":
            return ev
        return ch

    def _make_item(
        self,
        item_num,
        text,
        register,
        gold,
        rules,
        primary_family,
        business_decision,
        geo_stratum,
        uf,
        reg_ids,
        mt_context,
        other_uf_regional,
        scenario_family_id,
        surface_skeleton_id,
        novelty_reason,
        capability_requirement,
        source_refs,
        rationale,
        split,
        batch_id,
        context=None,
    ):
        final_text = self.ensure_unique_text(text, item_num, register)
        prov = "synthetic_regional_grounded" if (mt_context or other_uf_regional) else "synthetic_business_scenario"
        all_sources = list(
            dict.fromkeys(
                ["RFB-CNPJ-2026-10", "IBGE-MUNI-202610"]
                + source_refs
                + [r for r in reg_ids if r.startswith("REG-") or r.startswith("EXT-")]
            )
        )
        row = {
            "id": f"{ID_PREFIX}-{item_num:05d}",
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

    # -- single-turn generators -----------------------------------------
    def _gen_single_item(self, fam, unit_idx, local_idx, split, geo_stratum, register, batch_id, item_num):
        seed = unit_idx * 1009 + local_idx * 37 + item_num + 999999
        uf, place_ir, place_phrase, mt_ctx, ouf_reg, reg_ids, opener, muni_obj = self._resolve_geo(geo_stratum, seed)
        prefix_used = self._geo_prefix
        s = self.pick_sector(seed)
        sname, splural, colloquial = self.sector_surface(s, seed)
        novelty_prefix = f"colloquial_sector:{sname}" if colloquial else ("municipal_prefix_exact_match" if prefix_used else None)

        def novelty(extra=None):
            if extra:
                return extra
            return novelty_prefix

        if fam == "market_size_density_competition":
            sub = local_idx % 5
            if sub == 0:
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas existem {place_phrase} para medir a concorrência local?",
                    f"{opener}Me diga quantas {splural} ativas estão {place_phrase} no retrato cadastral atual.",
                    f"{opener}{cap(place_phrase)}, quantas {splural} ativas o cadastro mostra?",
                )
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06", "SR-07"], fam, "Size active market stock in target territory", geo_stratum, uf,
                    reg_ids, mt_ctx, ouf_reg, f"mkt_stock_{sub}", f"COUNT/STOCK/SECTOR/PLACE/{sub}", novelty(),
                    "supported_aggregates", [], "Active establishment stock counted for an explicit sector and place.", split, batch_id)
            if sub == 1:
                if place_ir == "any":
                    place_ir, place_phrase, uf, mt_ctx = place_uf("MT"), "no estado de Mato Grosso", "MT", True
                use_census = local_idx % 2 == 1
                pop_ref = {"ibge_census": 2022} if use_census else {"ibge_estimate": 2026}
                pop_txt = "pelo censo IBGE 2022" if use_census else "pela estimativa IBGE 2026"
                text = self.frames(
                    split,
                    f"{opener}Qual a densidade por 10 mil habitantes de {splural} ativas {place_phrase} {pop_txt}?",
                    f"{opener}Informe quantas {splural} ativas existem por 10 mil habitantes {place_phrase}, {pop_txt}.",
                    f"{opener}Calcule a taxa por 10 mil habitantes para {splural} ativas {place_phrase} {pop_txt}.",
                )
                ir = base_ir({"kind": "count", "metric": {"per10k": pop_ref}}, place_ir, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06", "SR-11"], fam, "Evaluate market saturation per 10,000 inhabitants", geo_stratum, uf,
                    reg_ids, mt_ctx, ouf_reg, f"mkt_per10k_{use_census}", f"COUNT/PER10K/SECTOR/PLACE/{use_census}", novelty(),
                    "supported_aggregates", [], "Density per 10,000 inhabitants with an explicit IBGE population reference.", split, batch_id)
            if sub == 2:
                if place_ir == "any":
                    place_ir, place_phrase, uf, mt_ctx = place_uf("MT"), "em Mato Grosso", "MT", True
                text = self.frames(
                    split,
                    f"{opener}Quantos habitantes por estabelecimento ativo de {sname} há {place_phrase}, pela estimativa IBGE 2026?",
                    f"{opener}Informe os habitantes por estabelecimento ativo de {sname} {place_phrase} segundo a estimativa IBGE 2026.",
                    f"{opener}Calcule habitantes por estabelecimento ativo de {sname} {place_phrase} usando a estimativa populacional IBGE 2026.",
                )
                ir = base_ir({"kind": "count", "metric": {"inhabitants_per": {"ibge_estimate": 2026}}}, place_ir, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06", "SR-11"], fam, "Measure inhabitants served per active establishment", geo_stratum, uf,
                    reg_ids, mt_ctx, ouf_reg, "mkt_inhab_per", "COUNT/INHAB_PER/SECTOR/PLACE", novelty(),
                    "supported_aggregates", [], "Inhabitants per active establishment using the IBGE 2026 estimate.", split, batch_id)
            if sub == 3:
                target_uf = "MT" if uf in ("MT", "BR", "MULTI") else uf
                if target_uf == "MT":
                    mt_ctx = True
                k = [5, 10, 15][(local_idx // 5) % 3]
                pop_thresh = [20000, 30000, 50000][(local_idx // 3) % 3]
                text = self.frames(
                    split,
                    f"{opener}Quais os top {k} municípios com mais {splural} ativas em {target_uf}, considerando apenas cidades com mais de {pop_thresh} habitantes?",
                    f"{opener}Quais os {k} municípios de {target_uf} com maior número de {splural} ativas entre cidades acima de {pop_thresh} habitantes?",
                    f"{opener}Quais os top {k} municípios de {target_uf} com mais {splural} ativas, só cidades com mais de {pop_thresh} habitantes?",
                )
                extra = {"place_population": {"from": pop_thresh + 1, "to": None}, "population_ref": {"ibge_estimate": 2026}}
                ir = base_ir({"kind": "rank", "dim": "municipality", "metric": "stock", "dir": "desc", "k": k, "min_base": 0},
                    place_uf(target_uf), activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "rank", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07", "SR-11"], fam, "Rank municipalities in state filtered by minimum population", geo_stratum,
                    target_uf, reg_ids, mt_ctx, ouf_reg, "mkt_rank_popband", "RANK/MUNI/STOCK/POP_BAND", novelty(),
                    "supported_aggregates", [], "Municipality ranking by active stock restricted to municipalities above a population threshold.", split, batch_id)
            if local_idx % 2 == 0:
                text = self.frames(
                    split,
                    f"Nessa região, {splural} é mato ou ainda cabe mais uma {sname}?",
                    f"O mercado de {sname} {place_phrase} tá saturado ou tem espaço pra mais?",
                    f"Dizem que {sname} {place_phrase} é mato; isso procede na praça?",
                )
                reg_list = list(dict.fromkeys(reg_ids + ["reg_mt_e_mato", "biz_amb_saturation", "REG-MT-03", "REG-BR-01"]))
            else:
                text = self.frames(
                    split,
                    f"Dá para dizer que {splural} {place_phrase} é mato, ou ainda tem demanda?",
                    f"O mercado de {sname} {place_phrase} já tá saturado ou cabe mais uma operação?",
                    f"Tem espaço para mais uma {sname} {place_phrase} ou o mercado está saturado?",
                )
                reg_list = list(dict.fromkeys(reg_ids + ["biz_amb_saturation", "REG-BR-01"]))
            gold = {"decision": "clarify", "intent": None, "ir": None,
                "expected_behavior": "Ask whether the user wants active establishment count, density per 10k inhabitants, or inhabitants per establishment."}
            return self._make_item(item_num, text, register, gold, ["SR-09", "SR-10"], fam,
                "Clarify subjective market saturation or regional abundance metaphor", geo_stratum, uf, reg_list, True, ouf_reg,
                "mkt_saturation_clarify", "CLARIFY/SATURATION_ABUNDANCE", novelty(),
                "clarify_ambiguous_quality", [], "Abundance/saturation predicates require clarifying the cadastral metric.", split, batch_id)

        if fam == "prospecting_qualification_territory":
            sub = local_idx % 8
            limit = [10, 15, 20, 25, 30, 50][local_idx % 6]
            c_code, c_phrase, c_short = CONTACT[local_idx % 3]
            if sub == 0:
                text = self.frames(
                    split,
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} que sejam matriz, microempresa e {c_phrase}.",
                    f"{opener}Liste {limit} {splural} ativas {place_phrase}, somente matrizes de microempresa {c_short}.",
                    f"{opener}Traga {limit} {splural} ativas {place_phrase} que sejam matriz e microempresa, {c_phrase}.",
                )
                extra = {"contact": c_code, "head_office": "yes", "porte": ["micro"]}
                order = "cnpj"
            elif sub == 1:
                text = self.frames(
                    split,
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} optantes pelo Simples Nacional, que não sejam MEI e {c_phrase}.",
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} do Simples Nacional, fora do MEI, {c_short}.",
                    f"{opener}Traga {limit} {splural} ativas {place_phrase} que sejam do Simples, não MEI, {c_phrase}.",
                )
                extra = {"contact": c_code, "simples": "yes", "mei": "no"}
                order = "cnpj"
            elif sub == 2:
                cap_reais = [50000, 100000, 250000, 500000][local_idx % 4]
                text = self.frames(
                    split,
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} que sejam matriz, {c_phrase} e com capital social acima de {cap_reais} reais, ordenados do maior capital para o menor.",
                    f"{opener}Liste {limit} matrizes de {sname} {place_phrase}, {c_short}, com capital social acima de {cap_reais} reais, do maior para o menor capital.",
                    f"{opener}Traga {limit} {splural} ativas {place_phrase} que sejam matriz e {c_phrase}, com capital social acima de {cap_reais} reais, ordenadas pelo maior capital social.",
                )
                extra = {"contact": c_code, "head_office": "yes", "capital": {"from": cap_reais * 100 + 1, "to": None}}
                order = "capital_desc"
            elif sub == 3:
                text = self.frames(
                    split,
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} de pequeno porte, {c_phrase} e com início de atividade em 2025, dos mais recentes primeiro.",
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} de pequeno porte {c_short}, com início de atividade em 2025, começando pelas mais novas.",
                    f"{opener}Traga {limit} {splural} ativas {place_phrase} de pequeno porte, {c_phrase}, com início de atividade em 2025, começando pelas mais novas.",
                )
                extra = {"contact": c_code, "porte": ["small"], "opened": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                order = "newest_first"
            elif sub == 4:
                text = self.frames(
                    split,
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} que sejam matriz, sociedade empresária limitada e {c_phrase}, das mais antigas para as mais recentes.",
                    f"{opener}Liste {limit} matrizes de {sname} {place_phrase}, LTDA e {c_short}, começando pelas mais antigas.",
                    f"{opener}Traga {limit} {splural} ativas {place_phrase} que sejam matriz, LTDA e {c_phrase}, das mais antigas para as mais novas.",
                )
                extra = {"contact": c_code, "head_office": "yes", "natureza": [{"code": 2062}]}
                order = "oldest_first"
            elif sub == 5:
                page = 2 if local_idx % 2 == 0 else 3
                text = self.frames(
                    split,
                    f"{opener}Traga a página {page} e liste {limit} {splural} ativas {place_phrase} optantes pelo MEI e {c_phrase}.",
                    f"{opener}Na página {page}, liste {limit} {splural} ativas {place_phrase} que sejam MEI, {c_short}.",
                    f"{opener}Traga a página {page} e liste {limit} {splural} ativas {place_phrase} do MEI, {c_phrase}.",
                )
                extra = {"contact": c_code, "mei": "yes"}
                ir = base_ir({"kind": "list", "order": "cnpj", "limit": limit}, place_ir, activity_sector(s), page=page, extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "list", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07", "SR-12"], fam, "Paginate a qualified prospecting list with contact and tax filters", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, f"prosp_page_{page}", f"LIST/PAGE_{page}/CONTACT_MEI", novelty(),
                    "supported_engine", [], "Prospecting list with explicit page, limit, MEI filter and contact presence.", split, batch_id)
            elif sub == 6:
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas {place_phrase} são matriz, de pequeno porte e optantes pelo Simples Nacional?",
                    f"{opener}Conte as {splural} ativas {place_phrase} que sejam matriz, pequeno porte e Simples.",
                    f"{opener}Quantas {splural} ativas {place_phrase} combinam matriz, pequeno porte e Simples Nacional?",
                )
                extra = {"head_office": "yes", "porte": ["small"], "simples": "yes"}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06", "SR-07"], fam, "Count qualified establishments combining head office, size and Simples regime",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "prosp_count_triple", "COUNT/STOCK/TRIPLE_HQ_PORTE_SIMPLES", novelty(),
                    "supported_aggregates", [], "Three-filter intersection count (head office + porte + Simples).", split, batch_id)
            elif sub == 7:
                cap_reais = [30000, 50000, 100000][local_idx % 3]
                text = self.frames(
                    split,
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} optantes pelo MEI, {c_phrase}, com capital social até {cap_reais} reais.",
                    f"{opener}Liste {limit} {splural} ativas {place_phrase} do MEI, {c_short}, com capital social de até {cap_reais} reais.",
                    f"{opener}Traga {limit} {splural} ativas {place_phrase} MEI, {c_short}, com capital social de até {cap_reais} reais.",
                )
                extra = {"contact": c_code, "mei": "yes", "capital": {"from": None, "to": cap_reais * 100}}
                ir = base_ir({"kind": "list", "order": "cnpj", "limit": limit}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "list", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06", "SR-07", "SR-12"], fam, "Build a MEI prospecting list under a share-capital ceiling",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "prosp_capital_ceiling", "LIST/MEI_CAPITAL_CEILING", novelty("capital_ceiling_mei_list"),
                    "supported_engine", [], "List with MEI, contact presence and an upper capital-social bound; capital is not revenue.", split, batch_id)
            ir = base_ir({"kind": "list", "order": order, "limit": limit}, place_ir, activity_sector(s), extra_scope=extra)
            return self._make_item(item_num, text, register, {"decision": "execute", "intent": "list", "ir": ir, "act": "new_query"},
                ["SR-02", "SR-03", "SR-06", "SR-07", "SR-12"], fam, "Build a qualified commercial prospecting list with a triple filter intersection",
                geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"prosp_triple_{sub}", f"LIST/{order.upper()}/TRIPLE_{sub}", novelty(),
                "supported_engine", [], "Prospecting list with three explicit cadastral filters and a deterministic sort order.", split, batch_id)

        if fam == "historical_stock_flows_seasonality":
            sub = local_idx % 6
            if sub == 0:
                open_year = 2025 if local_idx % 2 == 0 else 2024
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} tiveram baixa cadastral em 2026 com início de atividade em {open_year} {place_phrase}?",
                    f"{opener}Conte as {splural} que tiveram baixa cadastral em 2026 com início de atividade em {open_year} {place_phrase}.",
                    f"{opener}Quantas {splural} já tiveram baixa cadastral em 2026 com início de atividade em {open_year} {place_phrase}?",
                )
                extra = {"opened": {"from": day_from_ymd(open_year, 1, 1), "to": day_from_ymd(open_year, 12, 31)}}
                period = {"days": {"from": day_from_ymd(2026, 1, 1), "to": ANCHOR_DAY}}
                ir = base_ir({"kind": "count", "metric": {"flow": "closures"}}, place_ir, activity_sector(s), status=["closed"],
                    period=period, extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-04", "SR-05", "SR-06", "SR-10"], fam, "Measure cohort survival/closure of establishments opened in a specific year",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"hist_cohort_closure_{open_year}", "COUNT/CLOSURES/OPENED_COHORT", novelty("opened_to_closure_cohort_boundary"),
                    "supported_engine", [], "Closure count in 2026 restricted to the cohort opened in the stated year.", split, batch_id)
            if sub == 1:
                hist_ym = [(2024, 11), (2024, 12), (2025, 1), (2025, 3), (2025, 6), (2025, 9), (2025, 12), (2026, 3), (2026, 6), (2026, 9)][local_idx % 10]
                comp_id = comp_from_ym(hist_ym[0], hist_ym[1])
                ym_str = f"{hist_ym[0]:04d}-{hist_ym[1]:02d}"
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas existiam {place_phrase} na competência {ym_str}?",
                    f"{opener}Na competência {ym_str}, quantas {splural} estavam ativas {place_phrase}?",
                    f"{opener}Qual o estoque ativo de {splural} {place_phrase} na competência {ym_str}?",
                )
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), competence=comp_id)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-05", "SR-06"], fam, "Inspect historical monthly active stock at a past snapshot competence",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"hist_stock_{ym_str}", "COUNT/STOCK/HISTORICAL_COMP", novelty(),
                    "supported_aggregates", [], f"Historical active stock count at competence {ym_str}.", split, batch_id)
            if sub == 2:
                from_ym = [(2024, 11), (2025, 1), (2025, 6), (2025, 10)][local_idx % 4]
                from_comp = comp_from_ym(from_ym[0], from_ym[1])
                from_str = f"{from_ym[0]:04d}-{from_ym[1]:02d}"
                use_pct = local_idx % 2 == 1
                metric_phrase = "a variação percentual do estoque ativo" if use_pct else "a variação absoluta do estoque ativo"
                text = self.frames(
                    split,
                    f"{opener}Qual foi {metric_phrase} de {splural} {place_phrase} entre {from_str} e 2026-10?",
                    f"{opener}Entre {from_str} e 2026-10, qual {metric_phrase} de {splural} {place_phrase}?",
                    f"{opener}Calcule {metric_phrase} de {splural} {place_phrase} entre {from_str} e 2026-10.",
                )
                ir = base_ir({"kind": "count", "metric": {"active_change": {"pct": use_pct}}}, place_ir, activity_sector(s),
                    period={"competences": {"from": from_comp, "to": CURRENT_COMP}})
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-03", "SR-05", "SR-06"], fam, "Compute active stock change between historical and current competences",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"hist_active_change_{use_pct}", "COUNT/ACTIVE_CHANGE/COMP_RANGE", novelty(),
                    "supported_aggregates", [], "Active stock delta between two monthly snapshot competences.", split, batch_id)
            if sub == 3:
                periods = [
                    ("no 1 trimestre de 2025", day_from_ymd(2025, 1, 1), day_from_ymd(2025, 3, 31)),
                    ("no 2 trimestre de 2025", day_from_ymd(2025, 4, 1), day_from_ymd(2025, 6, 30)),
                    ("no 3 trimestre de 2025", day_from_ymd(2025, 7, 1), day_from_ymd(2025, 9, 30)),
                    ("no 4 trimestre de 2025", day_from_ymd(2025, 10, 1), day_from_ymd(2025, 12, 31)),
                    ("no primeiro semestre de 2025", day_from_ymd(2025, 1, 1), day_from_ymd(2025, 6, 30)),
                    ("no segundo semestre de 2025", day_from_ymd(2025, 7, 1), day_from_ymd(2025, 12, 31)),
                ]
                p_txt, d_from, d_to = periods[local_idx % len(periods)]
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} tiveram registros de início de atividade {p_txt} {place_phrase}?",
                    f"{opener}Conte os registros de início de atividade de {splural} {p_txt} {place_phrase}.",
                    f"{opener}Quantas {splural} começaram a atividade {p_txt} {place_phrase}?",
                )
                ir = base_ir({"kind": "count", "metric": {"flow": "registrations"}}, place_ir, activity_sector(s), status=ALL_STATUS,
                    period={"days": {"from": d_from, "to": d_to}})
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-04", "SR-05", "SR-06"], fam, "Track seasonal quarterly or semester business registrations", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, "hist_seasonal_reg", "COUNT/REGISTRATIONS/QUARTER_SEMESTER", novelty(),
                    "supported_aggregates", [], "Registration flow count over an explicit quarter or semester window.", split, batch_id)
            if sub == 5:
                ranges = [
                    ("de 01/03/2025 a 31/03/2025", (2025, 3, 1), (2025, 3, 31)),
                    ("de 01/06/2025 a 30/06/2025", (2025, 6, 1), (2025, 6, 30)),
                    ("de 01/09/2025 a 30/09/2025", (2025, 9, 1), (2025, 9, 30)),
                    ("de 01/01/2025 a 15/01/2025", (2025, 1, 1), (2025, 1, 15)),
                ]
                r_txt, r_from, r_to = ranges[local_idx % len(ranges)]
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} tiveram registros de início de atividade {r_txt} {place_phrase}?",
                    f"{opener}Conte os registros de início de atividade de {splural} {r_txt} {place_phrase}.",
                    f"{opener}Quantas {splural} começaram a atividade {r_txt} {place_phrase}?",
                )
                ir = base_ir({"kind": "count", "metric": {"flow": "registrations"}}, place_ir, activity_sector(s), status=ALL_STATUS,
                    period={"days": {"from": day_from_ymd(*r_from), "to": day_from_ymd(*r_to)}})
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-04", "SR-05", "SR-06"], fam, "Track registrations over an exact day-level date range", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, "hist_exact_day_range", "COUNT/REGISTRATIONS/EXACT_DAYS", novelty("exact_day_range_registrations"),
                    "supported_aggregates", [], "Registration flow over an exact DD/MM/YYYY range.", split, batch_id)
            flow_kinds = [
                ("registrations", "registros de início de atividade", ALL_STATUS),
                ("closures", "baixas cadastrais", ["closed"]),
                ("net_registrations", "saldo entre aberturas e baixas cadastrais", ALL_STATUS),
            ]
            f_code, f_txt, f_status = flow_kinds[local_idx % 3]
            text = self.frames(
                split,
                f"{opener}Mostre a evolução mensal em 2025 de {f_txt} de {splural} {place_phrase}.",
                f"{opener}Como evoluiu mês a mês em 2025 o indicador de {f_txt} de {splural} {place_phrase}?",
                f"{opener}Apresente a série mensal de 2025 para {f_txt} de {splural} {place_phrase}.",
            )
            ir = base_ir({"kind": "trend", "metric": f_code, "grain": "month"}, place_ir, activity_sector(s), status=f_status,
                period={"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}})
            return self._make_item(item_num, text, register, {"decision": "execute", "intent": "trend", "ir": ir, "act": "new_query"},
                ["SR-04", "SR-05", "SR-06"], fam, "Analyze monthly seasonality of registrations, closures or net flow", geo_stratum,
                uf, reg_ids, mt_ctx, ouf_reg, f"hist_trend_{f_code}", f"TREND/MONTH/{f_code.upper()}", novelty(),
                "supported_aggregates", [], "Monthly flow trend over 2025.", split, batch_id)

        if fam == "geography_catchments_comparisons":
            sub = local_idx % 7
            if sub == 0 and muni_obj is not None:
                steps = 1 if local_idx % 2 == 0 else 2
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas existem na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos municípios vizinhos em até {steps} passos?",
                    f"{opener}Quantas {splural} ativas existem na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos vizinhos em até {steps} passos?",
                    f"{opener}Qual o total de {splural} ativas na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos municípios vizinhos em até {steps} passos?",
                )
                near_place = {"near": {"center": muni_obj["id"], "rule": {"neighbours": steps}}}
                ir = base_ir({"kind": "count", "metric": "stock"}, near_place, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Size a contiguous municipal catchment for a regional sales route", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, f"geo_neighbours_{steps}", f"COUNT/STOCK/NEAR_NEIGHBOURS_{steps}", novelty(),
                    "supported_aggregates", [], "Active stock in a center municipality plus its contiguous neighbor ring.", split, batch_id)
            if sub == 1 and muni_obj is not None:
                km = [50, 80, 100, 120, 150][local_idx % 5]
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas existem na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos municípios próximos em um raio de {km} km?",
                    f"{opener}Na cidade de {muni_obj['name']}/{muni_obj['uf']} e nos municípios próximos em um raio de {km} km, quantas {splural} ativas há?",
                    f"{opener}Qual o total de {splural} ativas em {muni_obj['name']}/{muni_obj['uf']} e nos municípios próximos em um raio de {km} km?",
                )
                near_place = {"near": {"center": muni_obj["id"], "rule": {"within_km": km}}}
                ir = base_ir({"kind": "count", "metric": "stock"}, near_place, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Size a radial logistics catchment in kilometers around a hub city", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, f"geo_radius_{km}", f"COUNT/STOCK/NEAR_KM_{km}", novelty(),
                    "supported_aggregates", [], "Active stock within a centroid distance radius in kilometers.", split, batch_id)
            if sub == 2:
                if geo_stratum.startswith("mt_"):
                    m1 = muni_obj or self.next_mt_muni(geo_stratum, seed)
                    m2 = self.next_mt_muni(geo_stratum, seed + 1)
                    if m2["id"] == m1["id"]:
                        m2 = self.next_mt_muni("mt_metropolitan", seed + 2)
                elif geo_stratum == "other_26_ufs":
                    m1 = muni_obj or self.uf_munis[uf][0]
                    ul = self.uf_munis[uf]
                    m2 = ul[(seed + 1) % len(ul)]
                    if m2["id"] == m1["id"]:
                        m2 = ul[(seed + 2) % len(ul)] if len(ul) > 1 else self.mt_strata["mt_north_agro_industrial"][0]
                else:
                    m1 = self.mt_strata["mt_north_agro_industrial"][seed % len(self.mt_strata["mt_north_agro_industrial"])]
                    other_uf = NON_MT_UFS[seed % len(NON_MT_UFS)]
                    m2 = self.uf_munis[other_uf][seed % len(self.uf_munis[other_uf])]
                members = [{"municipality": mid} for mid in sorted({m1["id"], m2["id"]})]
                use_density = local_idx % 2 == 1
                metric = {"per10k": {"ibge_estimate": 2026}} if use_density else "stock"
                m_phrase = "a densidade por 10 mil habitantes" if use_density else "a quantidade"
                text = self.frames(
                    split,
                    f"{opener}Compare {m_phrase} de {splural} ativas entre {m1['name']}/{m1['uf']} e {m2['name']}/{m2['uf']}.",
                    f"{opener}Compare {splural} ativas por 10 mil habitantes entre {m1['name']}/{m1['uf']} e {m2['name']}/{m2['uf']}." if use_density else f"{opener}Compare a quantidade de {splural} ativas entre {m1['name']}/{m1['uf']} e {m2['name']}/{m2['uf']}.",
                    f"{opener}Compare {m_phrase} de {splural} entre {m1['name']}/{m1['uf']} e {m2['name']}/{m2['uf']}.",
                )
                ir = base_ir({"kind": "compare", "dim": "municipality", "members": members, "metric": metric}, "any", activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07", "SR-11"], fam, "Compare two commercial hub municipalities on stock or population-adjusted density",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"geo_compare_muni_{use_density}", f"COMPARE/MUNI/{'PER10K' if use_density else 'STOCK'}", novelty(),
                    "supported_aggregates", [], "Municipality comparison on an explicit metric and activity.", split, batch_id)
            if sub == 3:
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
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas existem em {target_uf}, exceto na cidade de {ex_m['name']}/{target_uf}?",
                    f"{opener}Quantas {splural} ativas existem em {target_uf}, exceto na cidade de {ex_m['name']}/{target_uf}, no retrato atual?",
                    f"{opener}No estado de {target_uf}, quantas {splural} ativas há, exceto na cidade de {ex_m['name']}/{target_uf}?",
                )
                extra = {"exclude": {"municipalities": [ex_m["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_uf(target_uf), activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Size the interior state market excluding one hub municipality", geo_stratum,
                    target_uf, reg_ids, mt_ctx, ouf_reg, "geo_exclude_muni", "COUNT/STOCK/UF_EXCLUDE_MUNI", novelty("state_internal_exclusion"),
                    "supported_aggregates", [], "UF stock count excluding the specified municipality.", split, batch_id)
            elif sub == 4:
                other_uf = uf if geo_stratum == "other_26_ufs" else NON_MT_UFS[local_idx % len(NON_MT_UFS)]
                uf1 = "MT" if (geo_stratum.startswith("mt_") or uf == "MT") else other_uf
                uf2 = "GO" if uf1 == "MT" and other_uf == "MT" else (other_uf if uf1 == "MT" else "MT")
                members = [{"uf": uid} for uid in sorted({UF_ID[uf1], UF_ID[uf2]})]
                text = self.frames(
                    split,
                    f"{opener}Compare a quantidade de {splural} ativas entre o estado de {UF_NAME[uf1]} e o estado de {UF_NAME[uf2]}.",
                    f"{opener}Compare {splural} ativas entre os estados de {UF_NAME[uf1]} e {UF_NAME[uf2]}.",
                    f"{opener}Faça a comparação de {splural} ativas entre os estados de {UF_NAME[uf1]} e {UF_NAME[uf2]}.",
                )
                ir = base_ir({"kind": "compare", "dim": "uf", "members": members, "metric": "stock"}, "any", activity_sector(s))
                item_uf = uf if geo_stratum == "other_26_ufs" else ("MT" if uf == "MT" else "MULTI")
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Compare two states on active establishments in a sector", geo_stratum, item_uf,
                    reg_ids, True, ouf_reg, "geo_compare_ufs", "COMPARE/UF/STOCK", novelty(), "supported_aggregates", [],
                    "State-to-state comparison on active establishment stock.", split, batch_id)
            elif sub == 5:
                if geo_stratum.startswith("mt_"):
                    m1 = muni_obj or self.next_mt_muni(geo_stratum, seed)
                    m2 = self.next_mt_muni(geo_stratum, seed + 1)
                    m3 = self.next_mt_muni(geo_stratum, seed + 2)
                elif geo_stratum == "other_26_ufs":
                    ul = self.uf_munis[uf]
                    m1 = muni_obj or ul[0]
                    m2 = ul[(seed + 1) % len(ul)]
                    m3 = ul[(seed + 2) % len(ul)]
                else:
                    m1 = self.mt_strata["mt_north_agro_industrial"][seed % len(self.mt_strata["mt_north_agro_industrial"])]
                    ou1 = NON_MT_UFS[seed % len(NON_MT_UFS)]
                    m2 = self.uf_munis[ou1][seed % len(self.uf_munis[ou1])]
                    ou2 = NON_MT_UFS[(seed + 5) % len(NON_MT_UFS)]
                    m3 = self.uf_munis[ou2][(seed + 3) % len(self.uf_munis[ou2])]
                uniq = []
                for mm in (m1, m2, m3):
                    if mm["id"] not in {x["id"] for x in uniq}:
                        uniq.append(mm)
                while len(uniq) < 3:
                    mm = self.next_mt_muni("mt_south_logistics_industrial", seed + len(uniq))
                    if mm["id"] not in {x["id"] for x in uniq}:
                        uniq.append(mm)
                for idx in range(3):
                    if uniq[idx]["folded"] in self.sector_surface_folded:
                        for step in range(1, 40):
                            mm = self.next_mt_muni("mt_south_logistics_industrial", seed + 100 + step)
                            if mm["id"] not in {x["id"] for x in uniq} and mm["folded"] not in self.sector_surface_folded:
                                uniq[idx] = mm
                                break
                members = [{"municipality": mm["id"]} for mm in sorted(uniq, key=lambda x: x["id"])]
                text = self.frames(
                    split,
                    f"{opener}Compare a quantidade de {splural} ativas entre {uniq[0]['name']}/{uniq[0]['uf']}, {uniq[1]['name']}/{uniq[1]['uf']} e {uniq[2]['name']}/{uniq[2]['uf']}.",
                    f"{opener}Coloque lado a lado {splural} ativas em {uniq[0]['name']}/{uniq[0]['uf']}, {uniq[1]['name']}/{uniq[1]['uf']} e {uniq[2]['name']}/{uniq[2]['uf']}.",
                    f"{opener}Faça a comparação de {splural} ativas entre três cidades: {uniq[0]['name']}/{uniq[0]['uf']}, {uniq[1]['name']}/{uniq[1]['uf']} e {uniq[2]['name']}/{uniq[2]['uf']}.",
                )
                ir = base_ir({"kind": "compare", "dim": "municipality", "members": members, "metric": "stock"}, "any", activity_sector(s))
                item_uf = uf if geo_stratum == "other_26_ufs" else ("MT" if geo_stratum.startswith("mt_") else "MULTI")
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Compare three municipalities on active sector stock", geo_stratum,
                    item_uf, reg_ids, True, ouf_reg, "geo_compare_three", "COMPARE/MUNI/THREE", novelty("three_city_comparison"),
                    "supported_aggregates", [], "Three-municipality stock comparison; descriptive only.", split, batch_id)
            else:
                if geo_stratum.startswith("mt_") or uf == "MT":
                    target_uf = "MT"
                    stratum = geo_stratum if geo_stratum.startswith("mt_") else "mt_metropolitan"
                    m1 = muni_obj or self.next_mt_muni(stratum, seed)
                    m2 = self.next_mt_muni(stratum, seed + 1)
                    mt_ctx = True
                elif geo_stratum == "other_26_ufs":
                    ul = self.uf_munis[uf]
                    target_uf = uf
                    if len(ul) < 2:
                        m1 = muni_obj or ul[0]
                        m2 = None
                    else:
                        m1 = muni_obj or ul[0]
                        m2 = ul[(seed + 1) % len(ul)]
                        if m2["id"] == m1["id"]:
                            m2 = ul[(seed + 2) % len(ul)]
                        if m2["folded"] in self.sector_surface_folded:
                            for step in range(2, min(len(ul), 40)):
                                cand = ul[step % len(ul)]
                                if cand["id"] != m1["id"] and cand["folded"] not in self.sector_surface_folded:
                                    m2 = cand
                                    break
                else:
                    target_uf = NON_MT_UFS[seed % len(NON_MT_UFS)]
                    ul = self.uf_munis[target_uf]
                    if len(ul) < 2:
                        m1 = ul[0]
                        m2 = None
                    else:
                        m1 = ul[seed % len(ul)]
                        m2 = ul[(seed + 1) % len(ul)]
                        if m2["id"] == m1["id"]:
                            m2 = ul[(seed + 2) % len(ul)]
                        if m2["folded"] in self.sector_surface_folded:
                            for step in range(2, min(len(ul), 40)):
                                cand = ul[step % len(ul)]
                                if cand["id"] != m1["id"] and cand["folded"] not in self.sector_surface_folded:
                                    m2 = cand
                                    break
                if m2 is None:
                    text = self.frames(
                        split,
                        f"{opener}Quantas {splural} ativas existem em {target_uf}, exceto na cidade de {m1['name']}/{target_uf}?",
                        f"{opener}Quantas {splural} ativas existem em {target_uf}, exceto na cidade de {m1['name']}/{target_uf}, no retrato atual?",
                        f"{opener}No estado de {target_uf}, quantas {splural} ativas há, exceto na cidade de {m1['name']}/{target_uf}?",
                    )
                    extra = {"exclude": {"municipalities": [m1["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}}
                    ir = base_ir({"kind": "count", "metric": "stock"}, place_uf(target_uf), activity_sector(s), extra_scope=extra)
                    return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                        ["SR-02", "SR-06", "SR-07"], fam, "Count a state market excluding the only municipality", geo_stratum,
                        target_uf, reg_ids, mt_ctx, ouf_reg, "geo_exclude_single_fallback", "COUNT/STOCK/UF_EXCLUDE_ONE", novelty("state_internal_exclusion"),
                        "supported_aggregates", [], "Fallback single-municipality exclusion for states with one catalogued municipality.", split, batch_id)
                text = self.frames(
                    split,
                    f"{opener}Quantas {splural} ativas existem em {target_uf}, exceto nas cidades de {m1['name']}/{target_uf} e {m2['name']}/{target_uf}?",
                    f"{opener}Quantas {splural} ativas existem em {target_uf}, exceto nas cidades de {m1['name']}/{target_uf} e {m2['name']}/{target_uf}, no retrato atual?",
                    f"{opener}No estado de {target_uf}, quantas {splural} ativas há, exceto nas cidades de {m1['name']}/{target_uf} e {m2['name']}/{target_uf}?",
                )
                extra = {"exclude": {"municipalities": sorted({m1["id"], m2["id"]}), "ufs": [], "sectors": [], "sector_cnaes": []}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_uf(target_uf), activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Count a state market excluding two municipalities", geo_stratum,
                    target_uf, reg_ids, mt_ctx, ouf_reg, "geo_exclude_two", "COUNT/STOCK/UF_EXCLUDE_TWO", novelty("two_municipality_exclusion"),
                    "supported_aggregates", [], "UF stock count excluding two explicit municipalities.", split, batch_id)

        if fam == "corporate_structure_hq_branches_identity":
            sub = local_idx % 5
            if sub == 0:
                text = self.frames(
                    split,
                    f"{opener}Quantas matrizes ativas de {sname} existem {place_phrase}?",
                    f"{opener}Quantas matrizes ativas de {sname} há {place_phrase}?",
                    f"{opener}Qual o número de matrizes ativas de {sname} {place_phrase}?",
                )
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), unit="head_offices")
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Count active head offices in the territory", geo_stratum, uf, reg_ids, mt_ctx,
                    ouf_reg, "corp_head_offices", "COUNT/STOCK/UNIT_HEAD_OFFICES", novelty(), "supported_aggregates", [],
                    "Head-office count uses unit='head_offices'.", split, batch_id)
            if sub == 1:
                text = self.frames(
                    split,
                    f"{opener}Quantas filiais ativas de {sname} existem {place_phrase}?",
                    f"{opener}Conte apenas as filiais ativas de {sname} {place_phrase}.",
                    f"{opener}Qual o número de filiais ativas de {sname} {place_phrase}?",
                )
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope={"head_office": "no"})
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Count active branch establishments in the territory", geo_stratum, uf, reg_ids,
                    mt_ctx, ouf_reg, "corp_branches_only", "COUNT/STOCK/BRANCHES_ONLY", novelty(), "supported_aggregates", [],
                    "Branch count sets head_office='no' on the establishment unit.", split, batch_id)
            if sub == 2:
                if local_idx % 2 == 0:
                    text = self.frames(
                        split,
                        f"{opener}Distribua as {splural} ativas {place_phrase} por matriz e filial, com percentual.",
                        f"{opener}Distribua as {splural} ativas {place_phrase} por matriz e filial, mostrando o percentual.",
                        f"{opener}Distribua as {splural} ativas {place_phrase} por matriz e filial, com o percentual de cada grupo.",
                    )
                    ir = base_ir({"kind": "breakdown", "dim": "head_office", "metric": "share"}, place_ir, activity_sector(s))
                else:
                    text = self.frames(
                        split,
                        f"{opener}Distribua as {splural} {place_phrase} em todas as situações por situação cadastral.",
                        f"{opener}Mostre a distribuição das {splural} {place_phrase} em todas as situações por situação cadastral.",
                        f"{opener}Qual a divisão das {splural} {place_phrase} em todas as situações por situação cadastral?",
                    )
                    ir = base_ir({"kind": "breakdown", "dim": "status", "metric": "stock"}, place_ir, activity_sector(s), status=ALL_STATUS)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Break down corporate structure by head office/branch or cadastral status",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"corp_breakdown_{local_idx % 2}", "BREAKDOWN/HQ_OR_STATUS", novelty(),
                    "supported_aggregates", [], "Breakdown by head_office or cadastral status.", split, batch_id)
            if sub == 3:
                pool = self.cnpjs["mt"] if (uf == "MT" or geo_stratum.startswith("mt_")) else self.cnpjs["other"]
                if split == "synthetic_dev":
                    c_entry = pool[(unit_idx * 4 + (local_idx // 5)) % 400]
                elif split == "synthetic_eval":
                    c_entry = pool[400 + ((unit_idx * 2 + (local_idx // 5)) % 120)]
                else:
                    c_entry = pool[520 + ((unit_idx + (local_idx // 5)) % 80)]
                use_root = local_idx % 2 == 1
                cid = c_entry["root"] if use_root else c_entry["full"]
                label_kind = "a raiz de CNPJ" if use_root else "o CNPJ"
                text = self.frames(
                    split,
                    f"{opener}Confira {label_kind} {cid} na base cadastral para conferência societária.",
                    f"{opener}Consulte {label_kind} {cid} no cadastro atual e descreva o registro.",
                    f"{opener}{cap(label_kind)} {cid}: o que consta no cadastro?",
                )
                ir = base_ir({"kind": "lookup", "id": cid}, "any", "any", status=ALL_STATUS)
                item_uf = uf if geo_stratum == "other_26_ufs" else c_entry["uf"]
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "lookup", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-13"], fam, "Verify an exact CNPJ or 8-digit company root in the snapshot", geo_stratum, item_uf,
                    reg_ids, mt_ctx, ouf_reg, f"corp_lookup_{use_root}", "LOOKUP/VALID_CNPJ", novelty(), "supported_engine", [],
                    "Exact lookup of a verified CNPJ or 8-digit root.", split, batch_id)
            st_variants = [
                (["suspended", "unfit", "closed"], "que não estão ativas"),
                (["suspended"], "suspensas"),
                (["unfit"], "inaptas"),
                (["closed"], "baixadas"),
            ]
            st_val, st_phrase = st_variants[(local_idx // 5) % 4]
            text = self.frames(
                split,
                f"{opener}Quantas {splural} {st_phrase} constam no cadastro {place_phrase}?",
                f"{opener}Quantas {splural} {st_phrase} constam no retrato cadastral {place_phrase}?",
                f"{opener}Qual o total de {splural} {st_phrase} no cadastro {place_phrase}?",
            )
            ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), status=st_val)
            return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                ["SR-03", "SR-06", "SR-07"], fam, "Audit non-active cadastral statuses in the territory", geo_stratum, uf, reg_ids,
                mt_ctx, ouf_reg, "corp_non_active_status", "COUNT/STOCK/NON_ACTIVE_STATUS", novelty(), "supported_aggregates", [],
                "Count of establishments with an explicit non-active cadastral status.", split, batch_id)

        if fam == "supply_chain_complementary":
            sub = local_idx % 5
            s1, s2 = self.supply_pairs[(unit_idx * 11 + local_idx) % len(self.supply_pairs)]
            a1, a1p, c1 = self.sector_surface(s1, seed + 1, force=True)
            a2, a2p, c2 = self.sector_surface(s2, seed + 2, force=True)
            coll = f"supply:{a1}+{a2}" if (c1 or c2) else None
            if sub == 0:
                members = [{"sector": sid} for sid in sorted({s1["id"], s2["id"]})]
                text = self.frames(
                    split,
                    f"{opener}Para mapear a cadeia produtiva {place_phrase}: compare a quantidade de {a1p} ativas e de {a2p} ativas.",
                    f"{opener}Compare as quantidades de {a1p} ativas e de {a2p} ativas {place_phrase}.",
                    f"{opener}Faça a comparação da cadeia {place_phrase} entre {a1p} e {a2p}.",
                )
                ir = base_ir({"kind": "compare", "dim": "sector", "members": members, "metric": "stock"}, place_ir, "any")
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Compare upstream and downstream supply-chain sectors in the same territory",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "supply_compare_stock", "COMPARE/SECTOR/STOCK", coll,
                    "supported_aggregates", [], "Sector-dimension comparison between two complementary supply-chain activities in the same place.", split, batch_id)
            if sub == 1:
                members = [{"sector": sid} for sid in sorted({s1["id"], s2["id"]})]
                text = self.frames(
                    split,
                    f"{opener}Na cadeia comercial {place_phrase}: compare os registros de início de atividade em 2025 entre {a1p} e {a2p}.",
                    f"{opener}Compare os registros de início de atividade de 2025 entre {a1p} e {a2p} {place_phrase}.",
                    f"{opener}Compare os registros de início de atividade em 2025 entre {a1p} e {a2p} {place_phrase}.",
                )
                period = {"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                ir = base_ir({"kind": "compare", "dim": "sector", "members": members, "metric": {"flow": "registrations"}}, place_ir,
                    "any", status=ALL_STATUS, period=period)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "compare", "ir": ir, "act": "new_query"},
                    ["SR-04", "SR-06", "SR-07"], fam, "Compare new business entry across two complementary supply-chain links",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "supply_compare_reg", "COMPARE/SECTOR/REGISTRATIONS", coll,
                    "supported_aggregates", [], "Sector comparison on 2025 registration flows.", split, batch_id)
            if sub == 2:
                use_any_cnae = local_idx % 2 == 1
                cnae_scope = "any" if use_any_cnae else "main"
                sec_phrase = "incluindo CNAE secundário" if use_any_cnae else "no CNAE principal"
                text = self.frames(
                    split,
                    f"{opener}Quantas {a1p} e {a2p} ativas ({sec_phrase}) existem {place_phrase}?",
                    f"{opener}Quantas {a1p} e {a2p} ativas ({sec_phrase}) existem {place_phrase}?",
                    f"{opener}Qual o total combinado de {a1p} e {a2p} ativas ({sec_phrase}) {place_phrase}?",
                )
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_two_sectors(s1, s2, cnae_scope))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Count a combined supply-chain ecosystem across complementary sectors",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"supply_union_{cnae_scope}", f"COUNT/STOCK/TWO_SECTORS_{cnae_scope.upper()}",
                    coll, "supported_engine" if use_any_cnae else "supported_aggregates", [],
                    "Combined count across two complementary sectors with explicit primary/secondary CNAE scope.", split, batch_id)
            if sub == 3:
                k = [5, 10][local_idx % 2]
                text = self.frames(
                    split,
                    f"{opener}Quais os top {k} setores com mais estabelecimentos ativos {place_phrase}?",
                    f"{opener}Quais os {k} setores com maior estoque ativo {place_phrase}?",
                    f"{opener}Quais os top {k} setores com mais estabelecimentos ativos {place_phrase}?",
                )
                ir = base_ir({"kind": "rank", "dim": "sector", "metric": "stock", "dir": "desc", "k": k, "min_base": 0}, place_ir, "any")
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "rank", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Identify dominant economic sectors in the target territory", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, "supply_rank_sectors", "RANK/SECTOR/STOCK", coll, "supported_aggregates", [],
                    "Ranking of curated sectors by active establishment count in the territory.", split, batch_id)
            use_div = local_idx % 2 == 1
            dim_val = {"cnae": "division" if use_div else "section"}
            dim_txt = "por divisão CNAE" if use_div else "por seção CNAE"
            text = self.frames(
                split,
                f"{opener}Distribua os estabelecimentos ativos {place_phrase} {dim_txt}.",
                f"{opener}Distribua os estabelecimentos ativos {place_phrase} {dim_txt}, listando cada grupo.",
                f"{opener}Distribua os estabelecimentos ativos {place_phrase} {dim_txt}, mostrando cada grupo.",
            )
            ir = base_ir({"kind": "breakdown", "dim": dim_val, "metric": "stock"}, place_ir, "any")
            return self._make_item(item_num, text, register, {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], fam, "Map the full CNAE section or division profile of the local economy", geo_stratum,
                uf, reg_ids, mt_ctx, ouf_reg, f"supply_cnae_breakdown_{use_div}", "BREAKDOWN/CNAE_LEVEL", "cnae_section_division_breakdown",
                "supported_aggregates", [], "Breakdown of active establishments across CNAE sections or divisions.", split, batch_id)

        if fam == "cross_variable_correlation":
            sub = local_idx % 10
            if sub == 0:
                target_uf = "MT" if (uf in ("MT", "BR", "MULTI") or geo_stratum.startswith("mt_")) else uf
                if target_uf == "MT":
                    mt_ctx = True
                pop_min = [25000, 40000, 60000][local_idx % 3]
                text = self.frames(
                    split,
                    f"{opener}Para cruzar porte populacional do IBGE e presença setorial: quais os top 10 municípios com maior densidade por 10 mil habitantes de {splural} em {target_uf} entre cidades com mais de {pop_min} habitantes?",
                    f"{opener}Cruzando população e {splural}: quais os 10 municípios de {target_uf} com maior densidade por 10 mil habitantes, só cidades acima de {pop_min} habitantes?",
                    f"{opener}Quais os top 10 municípios de {target_uf} com maior densidade por 10 mil habitantes de {splural} entre cidades com mais de {pop_min} habitantes?",
                )
                extra = {"place_population": {"from": pop_min + 1, "to": None}, "population_ref": {"ibge_estimate": 2026}}
                ir = base_ir({"kind": "rank", "dim": "municipality", "metric": {"per10k": {"ibge_estimate": 2026}}, "dir": "desc", "k": 10, "min_base": 0},
                    place_uf(target_uf), activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "rank", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-11"], fam, "Cross-reference a municipal population threshold with per-10k sector density",
                    geo_stratum, target_uf, reg_ids, mt_ctx, ouf_reg, "corr_supported_pop_density_rank", "RANK/MUNI/PER10K/POP_BAND", novelty("population_band_density_rank"),
                    "supported_aggregates", [], "Supported cross-variable analysis combining an IBGE population band with per-10k density ranking.", split, batch_id)
            if sub == 1:
                text = self.frames(
                    split,
                    f"{opener}Para cruzar fluxo de entrada e saída cadastral: qual o saldo entre aberturas e baixas de {splural} em 2025 {place_phrase}?",
                    f"{opener}Em 2025, qual o saldo entre aberturas e baixas cadastrais de {splural} {place_phrase}?",
                    f"{opener}Calcule o saldo de registros menos baixas de {splural} em 2025 {place_phrase}.",
                )
                period = {"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
                ir = base_ir({"kind": "count", "metric": {"flow": "net_registrations"}}, place_ir, activity_sector(s), status=ALL_STATUS, period=period)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-04", "SR-05", "SR-06"], fam, "Compare registration inflows against closure outflows via net flow balance",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "corr_supported_net_flow", "COUNT/NET_REGISTRATIONS/YEAR", novelty(),
                    "supported_aggregates", [], "Net registration balance (registrations minus closures) over 2025.", split, batch_id)
            if sub == 2:
                use_mei = local_idx % 2 == 1
                dim_code = "mei" if use_mei else "porte"
                dim_txt = "por opção pelo MEI" if use_mei else "por porte cadastral"
                text = self.frames(
                    split,
                    f"{opener}Para relacionar perfil tributário/porte e presença comercial: distribua as {splural} ativas {place_phrase} {dim_txt}, com percentual.",
                    f"{opener}Mostre a participação percentual das {splural} ativas {place_phrase} {dim_txt}.",
                    f"{opener}Qual a divisão percentual das {splural} ativas {place_phrase} {dim_txt}?",
                )
                ir = base_ir({"kind": "breakdown", "dim": dim_code, "metric": "share"}, place_ir, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Analyze the joint distribution of sector activity across company size or MEI regime",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"corr_supported_share_{dim_code}", f"BREAKDOWN/{dim_code.upper()}/SHARE", novelty("sector_by_tax_regime_share"),
                    "supported_aggregates", [], "Percentage share breakdown of sector establishments by size or MEI status.", split, batch_id)
            if sub == 3:
                ext_mode = local_idx % 3
                if ext_mode == 0:
                    text = f"{opener}Qual a correlação entre a produção de soja em toneladas da PAM do IBGE e o total de {splural} ativas {place_phrase}?"
                    ext_src = ["EXT-IBGE-PAM"]
                    beh = "Reject: IBGE PAM agricultural production tonnage and statistical correlation are not in the loaded cadastral/population artifacts."
                elif ext_mode == 1:
                    text = f"{opener}Cruze os dados do CEMPRE do IBGE de pessoal ocupado assalariado e salário médio com as {splural} ativas {place_phrase}."
                    ext_src = ["EXT-IBGE-CEMPRE"]
                    beh = "Reject: IBGE CEMPRE headcount and wage mass tables are not integrated into the runtime snapshot/aggregate artifacts."
                else:
                    text = f"{opener}Qual a razão entre o efetivo de rebanho bovino da PPM do IBGE e o número de {splural} ativas {place_phrase}?"
                    ext_src = ["EXT-IBGE-PPM"]
                    beh = "Reject: IBGE PPM livestock herd counts are not loaded in the cadastral/population runtime."
                return self._make_item(item_num, text, register, {"decision": "reject", "intent": None, "ir": None, "expected_behavior": beh},
                    ["SR-01", "SR-09", "SR-10"], fam, "Fail closed when asked for unintegrated external IBGE CEMPRE/PAM/PPM series", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, f"corr_reject_external_{ext_mode}", f"REJECT/EXTERNAL_IBGE_{ext_mode}", novelty(),
                    "reject_external_cempre_pam_ppm", ext_src, beh, split, batch_id)
            if sub == 4:
                cap_reais = [50000, 100000, 250000][local_idx % 3]
                porte_txt, porte_val = [("microempresa", ["micro"]), ("pequeno porte", ["small"])][local_idx % 2]
                text = self.frames(
                    split,
                    f"{opener}Para cruzar capital social e porte declarado: quantas {splural} ativas {place_phrase} são {porte_txt} e têm capital social acima de {cap_reais} reais?",
                    f"{opener}Quantas {splural} ativas {place_phrase} combinam {porte_txt} com capital social acima de {cap_reais} reais?",
                    f"{opener}No cruzamento entre porte e capital, quantas {splural} ativas {place_phrase} são {porte_txt} com capital social acima de {cap_reais} reais?",
                )
                extra = {"porte": porte_val, "capital": {"from": cap_reais * 100 + 1, "to": None}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Cross declared size class with a bounded share-capital minimum",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "corr_capital_by_size", "COUNT/CAPITAL_BY_PORTE", novelty("capital_by_declared_size"),
                    "supported_engine", [], "Descriptive intersection of declared porte and capital social; capital is not revenue.", split, batch_id)
            if sub == 5:
                target_uf = "MT" if (uf in ("MT", "BR", "MULTI") or geo_stratum.startswith("mt_")) else uf
                if target_uf == "MT":
                    mt_ctx = True
                pop_min = [20000, 50000, 100000][local_idx % 3]
                text = self.frames(
                    split,
                    f"{opener}Para relacionar penetração de matrizes e população: quantas matrizes ativas de {sname} existem em {target_uf} em municípios com mais de {pop_min} habitantes?",
                    f"{opener}Quantas matrizes ativas de {sname} há em {target_uf}, considerando municípios com mais de {pop_min} habitantes?",
                    f"{opener}No cruzamento entre matrizes e porte populacional, quantas matrizes ativas de {sname} existem em {target_uf} em municípios com mais de {pop_min} habitantes?",
                )
                extra = {"place_population": {"from": pop_min + 1, "to": None}, "population_ref": {"ibge_estimate": 2026}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_uf(target_uf), activity_sector(s), unit="head_offices", extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-11"], fam, "Relate headquarters penetration to a municipal population band",
                    geo_stratum, target_uf, reg_ids, mt_ctx, ouf_reg, "corr_branch_penetration", "COUNT/HEAD_OFFICE_POP_BAND", novelty("branch_penetration_population"),
                    "supported_aggregates", [], "Descriptive head-office count restricted to municipalities above a population threshold; no unmet-demand claim.", split, batch_id)
            if sub == 6:
                text = self.frames(
                    split,
                    f"{opener}Para medir coocorrência de classificação: quantas empresas com {sname} também no CNAE secundário existem {place_phrase}?",
                    f"{opener}Quantas {splural} aparecem no CNAE secundário {place_phrase}?",
                    f"{opener}Qual o total de empresas que declaram {sname} no CNAE secundário {place_phrase}?",
                )
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s, scope="any"))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Measure main/secondary CNAE co-occurrence for a sector",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "corr_cnae_secondary", "COUNT/STOCK/CNAE_SECONDARY", novelty("cnae_secondary_cooccurrence"),
                    "supported_engine", [], "Count using secondary CNAE scope; declared classification is not actual procurement or demand.", split, batch_id)
            if sub == 7:
                stat_mode = local_idx % 2
                if stat_mode == 0:
                    text = f"{opener}Calcule o coeficiente de correlação de Pearson e o p-valor entre capital social médio e população nas {splural} {place_phrase}."
                else:
                    text = f"{opener}Rode uma regressão linear com R2 entre a taxa de abertura de {splural} e a renda per capita {place_phrase}."
                beh = "Reject: statistical regression, Pearson correlation coefficients, p-values, and municipal income series are outside supported query operations."
                return self._make_item(item_num, text, register, {"decision": "reject", "intent": None, "ir": None, "expected_behavior": beh},
                    ["SR-01", "SR-09", "SR-10"], fam, "Fail closed on unsupported statistical regression or Pearson correlation requests",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, f"corr_reject_stat_{stat_mode}", "REJECT/STATISTICAL_CORRELATION", novelty(),
                    "reject_statistical_correlation_or_forecast", [], beh, split, batch_id)
            if sub == 8:
                from_ym = [(2024, 11), (2025, 1), (2025, 6)][local_idx % 3]
                from_comp = comp_from_ym(from_ym[0], from_ym[1])
                from_str = f"{from_ym[0]:04d}-{from_ym[1]:02d}"
                text = self.frames(
                    split,
                    f"{opener}Para cruzar idade do cadastro e situação atual: qual a variação absoluta do estoque ativo de {splural} {place_phrase} entre {from_str} e 2026-10?",
                    f"{opener}Entre {from_str} e 2026-10, qual a variação do estoque ativo de {splural} {place_phrase}?",
                    f"{opener}Qual a variação absoluta do estoque ativo de {splural} {place_phrase} entre {from_str} e 2026-10?",
                )
                ir = base_ir({"kind": "count", "metric": {"active_change": {"pct": False}}}, place_ir, activity_sector(s),
                    period={"competences": {"from": from_comp, "to": CURRENT_COMP}})
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-03", "SR-05", "SR-06"], fam, "Relate cadastral age and current active status through stock change",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "corr_age_status_change", "COUNT/ACTIVE_CHANGE/AGE_STATUS", novelty("age_status_active_change"),
                    "supported_aggregates", [], "Descriptive active-stock change between a past competence and the current snapshot; no survival coefficient.", split, batch_id)
            if sub == 9:
                text = self.frames(
                    split,
                    f"{opener}Para medir concentração setorial: distribua as {splural} ativas {place_phrase} por seção CNAE.",
                    f"{opener}Distribua as {splural} ativas {place_phrase} por seção CNAE.",
                    f"{opener}Distribua as {splural} ativas {place_phrase} por seção CNAE, listando cada grupo.",
                )
                ir = base_ir({"kind": "breakdown", "dim": {"cnae": "section"}, "metric": "stock"}, place_ir, activity_sector(s))
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "breakdown", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-06", "SR-07"], fam, "Measure descriptive sector concentration through a CNAE-section breakdown",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "corr_concentration_cnae", "BREAKDOWN/CNAE_SECTION", novelty("concentration_cnae_section"),
                    "supported_aggregates", [], "Descriptive CNAE-section concentration; no market-share or profitability claim.", split, batch_id)
            raise ValueError(f"Unhandled correlation subvariant {sub}")

        if fam == "source_quality_provenance_uncertainty":
            sub = local_idx % 6
            if sub == 0:
                use_sources = local_idx % 2 == 1
                topic = "sources" if use_sources else "data_date"
                if use_sources:
                    text = self.frames(
                        split,
                        f"{opener}Quais fontes oficiais de dados são utilizadas para consultar {splural} ativas {place_phrase}?",
                        f"{opener}De onde vêm os dados oficiais usados na pesquisa de {splural} {place_phrase}?",
                        f"{opener}Liste as fontes oficiais por trás da consulta de {splural} {place_phrase}.",
                    )
                else:
                    text = self.frames(
                        split,
                        f"{opener}De que data e competência são os dados usados para pesquisar {splural} {place_phrase}?",
                        f"{opener}Qual a data de referência e a competência dos dados de {splural} {place_phrase}?",
                        f"{opener}Informe a competência e a data-âncora do retrato de {splural} {place_phrase}.",
                    )
                ir = base_ir({"kind": "meta", "topic": topic}, place_ir, activity_sector(s), status=ALL_STATUS)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "meta", "ir": ir}, ["SR-01", "SR-14"], fam,
                    "Verify data provenance, official sources and snapshot reference date", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    f"prov_meta_{topic}", f"META/{topic.upper()}", novelty(), "supported_static", [],
                    f"Meta query for {topic} preserving the mentioned place and sector scope.", split, batch_id)
            if sub == 1:
                use_meth = local_idx % 2 == 1
                topic = "methodology" if use_meth else "cnaes"
                if use_meth:
                    text = self.frames(
                        split,
                        f"{opener}Qual a metodologia usada para contar estabelecimentos ativos e baixas de {splural} {place_phrase}?",
                        f"{opener}Explique a metodologia de contagem de ativos e baixas para {splural} {place_phrase}.",
                        f"{opener}Como são contados ativos e baixas de {splural} {place_phrase}?",
                    )
                else:
                    text = self.frames(
                        split,
                        f"{opener}Quais CNAEs entram na pesquisa de {splural} {place_phrase}?",
                        f"{opener}Liste os CNAEs considerados para {splural} {place_phrase}.",
                        f"{opener}Quais códigos CNAE compõem a consulta de {splural} {place_phrase}?",
                    )
                ir = base_ir({"kind": "meta", "topic": topic}, place_ir, activity_sector(s), status=ALL_STATUS)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "meta", "ir": ir}, ["SR-01", "SR-07", "SR-14"],
                    fam, "Audit CNAE mapping and counting methodology before a commercial decision", geo_stratum, uf, reg_ids, mt_ctx,
                    ouf_reg, f"prov_meta_{topic}", f"META/{topic.upper()}", novelty(), "supported_static", [], f"Meta query for {topic}.", split, batch_id)
            if sub == 2:
                if local_idx % 3 == 0:
                    m = self.prefix_pool[(unit_idx * 7 + local_idx) % len(self.prefix_pool)]
                    text = self.frames(
                        split,
                        f"{opener}Quantas {splural} ativas existem na cidade de {m['name']}?",
                        f"{opener}Conte as {splural} ativas na cidade de {m['name']}.",
                        f"{opener}Qual o estoque ativo de {splural} na cidade de {m['name']}?",
                    )
                    ir = base_ir({"kind": "count", "metric": "stock"}, place_muni(m), activity_sector(s))
                    return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                        ["SR-02", "SR-06", "SR-08"], fam, "Resolve an exact municipality whose name is also a prefix of longer names",
                        geo_stratum, m["uf"], list(dict.fromkeys(reg_ids + ["REG-BR-01"])), m["uf"] == "MT", m["uf"] != "MT",
                        "prov_prefix_exact", "COUNT/STOCK/PREFIX_EXACT", "municipal_prefix_exact_match", "supported_aggregates", [],
                        "Exact single-word municipality name wins over longer prefix-sharing names.", split, batch_id)
                hname = self.homonym_names[(unit_idx + local_idx) % len(self.homonym_names)]
                text = f"Quantas {splural} ativas existem na cidade de {hname}?"
                gold = {"decision": "clarify", "intent": None, "ir": None,
                    "expected_behavior": f"Clarify which UF is intended for homonymous municipality {hname}."}
                item_uf = uf if geo_stratum == "other_26_ufs" else "UNRESOLVED"
                return self._make_item(item_num, text, register, gold, ["SR-08"], fam,
                    "Require UF disambiguation for a homonymous Brazilian/MT municipality", geo_stratum, item_uf, reg_ids,
                    hname in {"Água Boa", "Canarana", "Várzea Grande", "Pedra Preta", "Rio Branco", "Santa Terezinha", "Nova Olímpia", "General Carneiro", "Novo Santo Antônio"},
                    ouf_reg, "prov_clarify_homonym", "CLARIFY/HOMONYM_MUNI", "mt_and_national_homonym_municipality_guard", "clarify_homonym", [],
                    "A homonymous municipality without UF or session context requires clarification.", split, batch_id)
            if sub == 3:
                amb_mode = local_idx % 4
                if amb_mode == 0:
                    text = f"{opener}Quantas revendas agro ativas existem {place_phrase}?"
                    r_ids = list(dict.fromkeys(reg_ids + ["biz_amb_revenda_agro"]))
                elif amb_mode == 1:
                    text = f"{opener}Quantos estabelecimentos ativos de quebra-torto existem {place_phrase}?"
                    r_ids = list(dict.fromkeys(reg_ids + ["reg_mt_quebra_torto", "REG-MT-03"]))
                elif amb_mode == 2:
                    text = f"{opener}Quantas clínicas ativas existem {place_phrase}?"
                    r_ids = reg_ids
                else:
                    text = f"{opener}Quantas revendas ativas existem {place_phrase}?"
                    r_ids = reg_ids
                gold = {"decision": "clarify", "intent": None, "ir": None,
                    "expected_behavior": "Clarify which specific CNAE sector or activity is intended."}
                return self._make_item(item_num, text, register, gold, ["SR-07", "SR-09"], fam,
                    "Clarify an ambiguous commercial or regional activity umbrella term", geo_stratum, uf, r_ids, True if amb_mode < 2 else mt_ctx,
                    ouf_reg, f"prov_clarify_activity_{amb_mode}", "CLARIFY/AMBIGUOUS_ACTIVITY",
                    "regional_or_agro_ambiguous_activity_clarify", "clarify_ambiguous_activity", [],
                    "Umbrella or regional cultural terms without a specific CNAE sector require clarification.", split, batch_id)
            if sub == 4:
                if local_idx % 2 == 0:
                    bad_date = ["31/02/2025", "30/02/2025", "2025-13-01", "31/04/2025"][(local_idx // 2) % 4]
                    text = f"{opener}Quantas {splural} abriram {place_phrase} de {bad_date} a 31/05/2025?"
                    cap_req = "clarify_invalid_date_or_cnpj"
                    rat = "Impossible calendar date must fail closed with clarification."
                else:
                    text = f"{opener}Quantas {splural} ativas existiam {place_phrase} em 2025?"
                    cap_req = "clarify_missing_period"
                    rat = "Historical stock query for a full year without specifying a monthly competence requires clarification."
                gold = {"decision": "clarify", "intent": None, "ir": None, "expected_behavior": rat}
                return self._make_item(item_num, text, register, gold, ["SR-05", "SR-10"], fam,
                    "Detect an invalid calendar date or an underspecified historical snapshot competence", geo_stratum, uf, reg_ids,
                    mt_ctx, ouf_reg, f"prov_clarify_temporal_{local_idx % 2}", "CLARIFY/TEMPORAL_VALIDATION",
                    "temporal_validation_guard", cap_req, [], rat, split, batch_id)
            subreg = ["no Nortão de Mato Grosso", "na Baixada Cuiabana", "no Vale do Araguaia"][(unit_idx + local_idx) % 3]
            text = f"{opener}Quantas {splural} ativas existem {subreg}?"
            r_ids = list(dict.fromkeys(reg_ids + ["reg_mt_informal_subregions", "REG-MT-03"]))
            gold = {"decision": "clarify", "intent": None, "ir": None,
                "expected_behavior": "Clarify which specific municipalities or UF should be included for the informal subregion."}
            item_uf = uf if geo_stratum == "other_26_ufs" else "MT"
            return self._make_item(item_num, text, register, gold, ["SR-06", "SR-09"], fam,
                "Clarify an informal regional designation not mapped to a single catalogue entity", geo_stratum, item_uf, r_ids, True,
                ouf_reg, "prov_clarify_subregion", "CLARIFY/INFORMAL_SUBREGION", "mt_informal_subregion_clarification",
                "clarify_homonym", [], "Informal subregions require specifying the municipalities.", split, batch_id)

        if fam == "unsupported_missing_contradictions":
            sub = local_idx % 5
            if sub == 0:
                attr = ["o faturamento mensal médio", "o lucro líquido anual", "a folha de pagamento total", "a quantidade total de funcionários CLT"][local_idx % 4]
                text = f"{opener}Qual é {attr} das {splural} {place_phrase}?"
                gold = {"decision": "reject", "intent": None, "ir": None,
                    "expected_behavior": "Reject: revenue, profit, payroll, and employee headcount are absent from public RFB CNPJ data."}
                return self._make_item(item_num, text, register, gold, ["SR-09", "SR-10"], fam,
                    "Refuse unavailable financial or headcount attributes", geo_stratum, uf, reg_ids, mt_ctx, ouf_reg,
                    "unsup_financial_headcount", "REJECT/UNAVAILABLE_FINANCIAL", novelty(), "reject_unavailable_cadastral", [],
                    "Financial and employee headcount figures do not exist in public CNPJ data.", split, batch_id)
            if sub == 1:
                priv_mode = local_idx % 3
                if priv_mode == 0:
                    text = f"{opener}Traga o CPF e o telefone pessoal dos sócios das {splural} {place_phrase}."
                    r_ids = reg_ids
                elif priv_mode == 1:
                    text = f"{opener}Quantas {splural} {place_phrase} pertencem a sócios pau-rodados nascidos fora do estado?"
                    r_ids = list(dict.fromkeys(reg_ids + ["reg_mt_pau_rodado", "REG-MT-03"]))
                else:
                    text = f"{opener}Quem são os principais clientes e fornecedores das {splural} {place_phrase}?"
                    r_ids = list(dict.fromkeys(reg_ids + ["biz_amb_saturation"]))
                gold = {"decision": "reject", "intent": None, "ir": None,
                    "expected_behavior": "Reject: partner CPFs, personal phones, partner birthplace/origin, and private B2B client/supplier graphs are unavailable."}
                return self._make_item(item_num, text, register, gold, ["SR-09", "SR-10", "SR-12"], fam,
                    "Refuse private partner data, partner origin or B2B transaction graphs", geo_stratum, uf, r_ids,
                    True if priv_mode == 1 else mt_ctx, ouf_reg, f"unsup_privacy_or_b2b_{priv_mode}", "REJECT/PRIVACY_OR_CLIENTS",
                    novelty(), "reject_privacy_or_advice", [],
                    "Personal partner attributes and B2B client/supplier lists are outside public cadastral scope.", split, batch_id)
            if sub == 2:
                if local_idx % 2 == 0:
                    fut_year = 2027 if local_idx % 4 == 0 else 2028
                    text = f"{opener}Quantas {splural} vão abrir em {fut_year} {place_phrase}?"
                    beh = f"Reject: cannot forecast business registrations for future year {fut_year}."
                else:
                    text = f"{opener}Onde vale mais a pena investir para ter retorno rápido com {splural} {place_phrase}?"
                    beh = "Reject: investment return advice is outside factual cadastral search."
                return self._make_item(item_num, text, register, {"decision": "reject", "intent": None, "ir": None, "expected_behavior": beh},
                    ["SR-05", "SR-09", "SR-10"], fam, "Refuse future forecasts and subjective commercial investment advice", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, f"unsup_forecast_advice_{local_idx % 2}", "REJECT/FORECAST_OR_ADVICE", novelty(),
                    "reject_statistical_correlation_or_forecast", [], beh, split, batch_id)
            if sub == 3:
                sub_loc = ["no bairro Centro", "no CEP 78000-000", "no logradouro principal"][local_idx % 3]
                text = f"{opener}Quantas {splural} ativas existem {sub_loc} {place_phrase}?"
                gold = {"decision": "clarify", "intent": None, "ir": None,
                    "expected_behavior": "Clarify: neighborhood (bairro), CEP, and street (logradouro) filters are not supported; offer municipality-level search."}
                return self._make_item(item_num, text, register, gold, ["SR-06", "SR-09"], fam,
                    "Fail closed on an unsupported sub-municipal street/neighborhood/CEP filter", geo_stratum, uf, reg_ids, mt_ctx,
                    ouf_reg, "unsup_submunicipal_clarify", "CLARIFY/SUBMUNICIPAL_FILTER", novelty(), "clarify_ambiguous_quality", [],
                    "Sub-municipal filters (bairro, CEP, logradouro) fail closed with clarification.", split, batch_id)
            text = f"{opener}Quantas {splural} tiveram baixa cadastral com início de atividade em 2025 {place_phrase}?"
            gold = {"decision": "clarify", "intent": None, "ir": None,
                "expected_behavior": "Clarify the closure period: opening date ('início de atividade em 2025') is a cohort filter, not the closure date."}
            return self._make_item(item_num, text, register, gold, ["SR-04", "SR-05", "SR-10"], fam,
                "Require an explicit closure period when only the opening cohort period is stated", geo_stratum, uf, reg_ids, mt_ctx,
                ouf_reg, "unsup_closure_missing_period", "CLARIFY/CLOSURE_MISSING_EVENT_PERIOD", novelty(), "clarify_missing_period", [],
                "Opening period is a cohort filter and cannot substitute for the missing closure period.", split, batch_id)

        if fam == "tax_size_legal_capital_numeric":
            sub = local_idx % 5
            if sub == 0:
                cap_reais = [100000, 200000, 300000, 500000][local_idx % 4]
                text = f"{opener}Quantas {splural} ativas {place_phrase} são optantes pelo Simples Nacional, não são MEI e possuem capital social acima de {cap_reais} reais?"
                extra = {"simples": "yes", "mei": "no", "capital": {"from": cap_reais * 100 + 1, "to": None}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06", "SR-07"], fam, "Segment establishments by Simples regime, non-MEI status and minimum share capital",
                    geo_stratum, uf, reg_ids, mt_ctx, ouf_reg, "tax_simples_nonmei_cap", "COUNT/STOCK/SIMPLES_NONMEI_CAPITAL", novelty(),
                    "supported_engine", [], "Three-filter count combining Simples='yes', MEI='no' and a capital social lower bound.", split, batch_id)
            if sub == 1:
                use_sa = local_idx % 2 == 1
                nat_code = 2054 if use_sa else 2062
                nat_txt = "sociedade anônima fechada" if use_sa else "sociedade empresária limitada"
                text = f"{opener}Quantas {splural} ativas {place_phrase} possuem porte cadastral demais e natureza jurídica de {nat_txt}?"
                extra = {"porte": ["other"], "natureza": [{"code": nat_code}]}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Filter mid/large establishments (porte demais) by a specific legal nature", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, f"tax_porte_other_nat_{nat_code}", "COUNT/STOCK/PORTE_OTHER_NATUREZA", novelty(),
                    "supported_engine", [], "Count combining porte='other' (05) with an explicit legal nature code.", split, batch_id)
            if sub == 2:
                low_r = 50000
                high_r = [200000, 500000, 1000000][local_idx % 3]
                text = f"{opener}Quantas {splural} ativas {place_phrase} que são matriz possuem capital social entre {low_r} reais e {high_r} reais?"
                extra = {"head_office": "yes", "capital": {"from": low_r * 100, "to": high_r * 100}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Filter head offices within a bounded share capital bracket", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, "tax_hq_capital_between", "COUNT/STOCK/HQ_CAPITAL_BETWEEN", novelty(),
                    "supported_engine", [], "Count of active head offices with a bounded share capital interval in cents.", split, batch_id)
            if sub == 4:
                cap_reais = [50000, 100000, 250000][local_idx % 3]
                text = f"{opener}Quantas {splural} ativas {place_phrase} que são matriz têm capital social de até {cap_reais} reais?"
                extra = {"head_office": "yes", "capital": {"from": None, "to": cap_reais * 100}}
                ir = base_ir({"kind": "count", "metric": "stock"}, place_ir, activity_sector(s), extra_scope=extra)
                return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                    ["SR-02", "SR-03", "SR-06"], fam, "Filter head offices under a share-capital ceiling", geo_stratum,
                    uf, reg_ids, mt_ctx, ouf_reg, "tax_hq_capital_ceiling", "COUNT/STOCK/HQ_CAPITAL_CEILING", novelty("capital_ceiling_head_office"),
                    "supported_engine", [], "Count of active head offices with an upper share-capital bound; capital is not revenue.", split, batch_id)
            is_exit = local_idx % 2 == 1
            flow_code = "mei_exits" if is_exit else "mei_opt_ins"
            flow_txt = "exclusões do MEI" if is_exit else "opções pelo MEI"
            text = f"{opener}Quantas {splural} tiveram {flow_txt} em 2025 {place_phrase}?"
            period = {"days": {"from": day_from_ymd(2025, 1, 1), "to": day_from_ymd(2025, 12, 31)}}
            ir = base_ir({"kind": "count", "metric": {"flow": flow_code}}, place_ir, activity_sector(s), status=ALL_STATUS, period=period)
            return self._make_item(item_num, text, register, {"decision": "execute", "intent": "count", "ir": ir, "act": "new_query"},
                ["SR-04", "SR-05", "SR-06"], fam, "Track MEI tax regime entries or exits over a calendar year", geo_stratum, uf,
                reg_ids, mt_ctx, ouf_reg, f"tax_flow_{flow_code}", f"COUNT/{flow_code.upper()}/YEAR", novelty(), "supported_engine", [],
                f"Flow count of {flow_code} in 2025.", split, batch_id)

        raise ValueError(f"Unknown family: {fam}")

    # -- multi-turn dialogues -------------------------------------------
    def _gen_dialogue(self, unit_idx, d_idx, split, geos, regs, session_id, batch_id, start_item_num):
        fam = "dialogue_corrections_references_planning"
        seed = unit_idx * 503 + d_idx * 19 + start_item_num + 999999
        uf1, place1, phrase1, mt1, ouf1, reg1, opener1, m1 = self._resolve_geo(geos[0], seed)
        if m1 is None:
            m1 = self.next_mt_muni("mt_north_agro_industrial", seed)
            uf1, place1, phrase1, mt1 = "MT", place_muni(m1), f"na cidade de {m1['name']}/MT", True
        uf2, place2, phrase2, mt2, ouf2, reg2, _, m2 = self._resolve_geo(geos[1], seed + 7)
        if m2 is None or m2["id"] == m1["id"]:
            m2 = self.next_mt_muni("mt_south_logistics_industrial", seed + 11)
            uf2, place2, phrase2, mt2 = "MT", place_muni(m2), f"na cidade de {m2['name']}/MT", True
        s1 = self.pick_sector(seed)
        s1n, s1p, c1 = self.sector_surface(s1, seed)
        s2 = self.pick_sector(seed + 5)
        if s2["id"] == s1["id"]:
            s2 = self.pick_sector(seed + 9)
        s2n, s2p, c2 = self.sector_surface(s2, seed + 5)
        pattern = d_idx % 9
        novelty = "multi_turn_compound_scope_edit"
        turns = []

        def dialogue_item(turn, text, register, gold, rules, geo, item_uf, rids, mtc, oufr, scen, skel, cap_req, rationale, act=None):
            return self._make_item(
                start_item_num + turn, text, register, gold, rules, fam, "Resolve a multi-turn commercial planning dialogue",
                geo, item_uf, rids, mtc, oufr, scen, skel, novelty, cap_req, [], rationale, split, batch_id,
                context={"session_id": session_id, "turn": turn + 1},
            )

        if pattern == 0:
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[0], m1["uf"], reg1, mt1, ouf1, "dlg_p0_t1", "DLG/P0/T1_NEW",
                "supported_aggregates", "Dialogue turn 1: initial establishment count."))
            t2 = f"Agora só matrizes e sem MEI de {s1p} na cidade de {m1['name']}/{m1['uf']}."
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1), extra_scope={"head_office": "yes", "mei": "no"})
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "add_filter"},
                ["SR-02", "SR-03", "SR-06"], geos[1], m1["uf"], [], mt1, False, "dlg_p0_t2", "DLG/P0/T2_ADD_TWO_FILTERS",
                "supported_aggregates", "Dialogue turn 2: add head_office='yes' and mei='no' while preserving place and sector."))
            t3 = f"Na verdade mantenha o filtro de matriz, tire o MEI e troque para {s1p} na cidade de {m2['name']}/{m2['uf']}."
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m2), activity_sector(s1), extra_scope={"head_office": "yes"})
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "count", "ir": ir3, "act": "compound_edit"},
                ["SR-02", "SR-06", "SR-07"], geos[2], m2["uf"], reg2, mt2, ouf2, "dlg_p0_t3", "DLG/P0/T3_KEEP_DROP_SWITCH",
                "supported_aggregates", "Dialogue turn 3: keep the head-office filter, drop MEI and switch municipality."))
        elif pattern == 1:
            t1 = f"{opener1}Quantas {s1p} ativas que são matriz existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1), extra_scope={"head_office": "yes"})
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[0], m1["uf"], reg1, mt1, ouf1, "dlg_p1_t1", "DLG/P1/T1_NEW_HQ",
                "supported_aggregates", "Dialogue turn 1: count active head-office establishments."))
            t2 = f"Agora retire o filtro de matriz e conte matrizes e filiais de {s1p} na cidade de {m1['name']}/{m1['uf']}."
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "remove_filter"},
                ["SR-02", "SR-03"], geos[1], m1["uf"], [], mt1, False, "dlg_p1_t2", "DLG/P1/T2_REMOVE_ONE_FILTER",
                "supported_aggregates", "Dialogue turn 2: clear only the head_office restriction."))
            t3 = f"E para {s2p} ativas na cidade de {m1['name']}/{m1['uf']}?"
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s2))
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "count", "ir": ir3, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-07"], geos[2], m1["uf"], [], mt1, False, "dlg_p1_t3", "DLG/P1/T3_SECTOR_PIVOT",
                "supported_aggregates", "Dialogue turn 3: replace the sector while keeping the municipality."))
        elif pattern == 2:
            members = [{"municipality": mid} for mid in sorted({m1["id"], m2["id"]})]
            t1 = f"{opener1}Compare a quantidade de {s1p} ativas entre {m1['name']}/{m1['uf']} e {m2['name']}/{m2['uf']}."
            ir1 = base_ir({"kind": "compare", "dim": "municipality", "members": members, "metric": "stock"}, "any", activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "compare", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[0], m1["uf"], reg1, mt1, ouf1, "dlg_p2_t1", "DLG/P2/T1_COMPARE",
                "supported_aggregates", "Dialogue turn 1: two-municipality stock comparison."))
            t2 = f"E entre {m1['name']}/{m1['uf']} e {m2['name']}/{m2['uf']}, qual das duas tem mais {s1p} ativas por 10 mil habitantes?"
            ir2 = base_ir({"kind": "compare", "dim": "municipality", "members": members,
                "metric": {"per10k": {"ibge_estimate": 2026}}}, "any", activity_sector(s1))
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "compare", "ir": ir2, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-11"], geos[1], m1["uf"], [], mt1, False, "dlg_p2_t2", "DLG/P2/T2_FOCUS_PER10K",
                "supported_aggregates", "Dialogue turn 2: switch the comparison metric to per-10k density using the dialogue focus."))
            t3 = f"Quantas {s1p} ativas existem na cidade de {m2['name']}/{m2['uf']}?"
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m2), activity_sector(s1))
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "count", "ir": ir3, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[2], m2["uf"], reg2, mt2, ouf2, "dlg_p2_t3", "DLG/P2/T3_SINGLE_COUNT",
                "supported_aggregates", "Dialogue turn 3: pivot from comparison to a single-municipality count."))
        elif pattern == 3:
            short = self.prefix_pool[(unit_idx * 13 + d_idx) % len(self.prefix_pool)]
            longer = None
            for mm in self.munis.values():
                if mm["folded"] != short["folded"] and mm["folded"].startswith(short["folded"] + " "):
                    if sum(1 for x in self.munis.values() if x["folded"] == mm["folded"]) == 1:
                        longer = mm
                        break
            if longer is None:
                longer = self.next_mt_muni("mt_south_logistics_industrial", seed)
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {short['name']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(short), activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-08"], geos[0], short["uf"], reg1, short["uf"] == "MT", short["uf"] != "MT",
                "dlg_p3_t1", "DLG/P3/T1_PREFIX_EXACT", "supported_aggregates",
                "Dialogue turn 1: exact single-word municipality resolves before longer prefix-sharing names."))
            t2 = f"E na cidade de {longer['name']}/{longer['uf']}?"
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(longer), activity_sector(s1))
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-08"], geos[1], longer["uf"], [], longer["uf"] == "MT", longer["uf"] != "MT",
                "dlg_p3_t2", "DLG/P3/T2_LONGER_PREFIX", "supported_aggregates",
                "Dialogue turn 2: replace the place slot with the longer name sharing the same prefix."))
            members = [{"municipality": mid} for mid in sorted({short["id"], longer["id"]})]
            t3 = f"Agora compare as duas cidades, {short['name']} e {longer['name']}, para {s1p}."
            ir3 = base_ir({"kind": "compare", "dim": "municipality", "members": members, "metric": "stock"}, "any", activity_sector(s1))
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "compare", "ir": ir3, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-07"], geos[2], "MULTI", [], False, False, "dlg_p3_t3", "DLG/P3/T3_FOCUS_COMPARE",
                "supported_aggregates", "Dialogue turn 3: compare the two focus municipalities carried in conversation state."))
        elif pattern == 4:
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[0], m1["uf"], reg1, mt1, ouf1, "dlg_p4_t1", "DLG/P4/T1_NEW",
                "supported_aggregates", "Dialogue turn 1: initial municipality count."))
            t2 = f"Agora considere o estado de MT inteiro, exceto a cidade de {m1['name']}/MT."
            extra = {"exclude": {"municipalities": [m1["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}}
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_uf("MT"), activity_sector(s1), extra_scope=extra)
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-07"], geos[1], "MT", [], True, False, "dlg_p4_t2", "DLG/P4/T2_STATE_EXCLUDE_CITY",
                "supported_aggregates", "Dialogue turn 2: replace the place with the whole state and exclude the hub city."))
            t3 = f"Agora liste 20 matrizes de {s1p} com telefone no estado de MT, exceto {m1['name']}/MT."
            ir3 = base_ir({"kind": "list", "order": "cnpj", "limit": 20}, place_uf("MT"), activity_sector(s1),
                extra_scope={"head_office": "yes", "contact": "phone", "exclude": {"municipalities": [m1["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}})
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "list", "ir": ir3, "act": "add_filter"},
                ["SR-02", "SR-03", "SR-06", "SR-07", "SR-12"], geos[2], "MT", [], True, False, "dlg_p4_t3", "DLG/P4/T3_LIST_HQ_PHONE",
                "supported_engine", "Dialogue turn 3: turn the follow-up into a head-office phone-qualified list while keeping the state exclusion."))
        elif pattern == 5:
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[0], m1["uf"], reg1, mt1, ouf1, "dlg_p5_t1", "DLG/P5/T1_NEW",
                "supported_aggregates", "Dialogue turn 1: initial municipality count."))
            t2 = f"Agora só matrizes de microempresa optantes pelo Simples de {s1p} na cidade de {m1['name']}/{m1['uf']}."
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1),
                extra_scope={"head_office": "yes", "porte": ["micro"], "simples": "yes"})
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "add_filter"},
                ["SR-02", "SR-03", "SR-06"], geos[1], m1["uf"], [], mt1, False, "dlg_p5_t2", "DLG/P5/T2_ADD_THREE_FILTERS",
                "supported_aggregates", "Dialogue turn 2: add head-office, micro-enterprise and Simples filters."))
            t3 = f"Na verdade mantenha o Simples, tire o porte de microempresa e troque para {s1p} na cidade de {m2['name']}/{m2['uf']}."
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m2), activity_sector(s1),
                extra_scope={"head_office": "yes", "simples": "yes"})
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "count", "ir": ir3, "act": "compound_edit"},
                ["SR-02", "SR-03", "SR-06", "SR-07"], geos[2], m2["uf"], reg2, mt2, ouf2, "dlg_p5_t3", "DLG/P5/T3_KEEP_SIMPLES_DROP_PORTE_SWITCH",
                "supported_aggregates", "Dialogue turn 3: keep Simples, drop the porte filter and switch municipality."))
        elif pattern == 6:
            short = self.prefix_pool[(unit_idx * 17 + d_idx) % len(self.prefix_pool)]
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {short['name']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(short), activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-08"], geos[0], short["uf"], reg1, short["uf"] == "MT", short["uf"] != "MT",
                "dlg_p6_t1", "DLG/P6/T1_PREFIX_EXACT", "supported_aggregates",
                "Dialogue turn 1: exact single-word municipality resolves before longer prefix-sharing names."))
            t2 = f"Agora conte o estado de {short['uf']} inteiro, exceto a cidade de {short['name']}/{short['uf']}."
            extra = {"exclude": {"municipalities": [short["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}}
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_uf(short["uf"]), activity_sector(s1), extra_scope=extra)
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "replace_slot"},
                ["SR-02", "SR-06", "SR-07"], geos[1], short["uf"], [], short["uf"] == "MT", short["uf"] != "MT",
                "dlg_p6_t2", "DLG/P6/T2_STATE_EXCLUDE_PREFIX", "supported_aggregates",
                "Dialogue turn 2: expand to the state while excluding the exact prefix municipality."))
            t3 = f"E liste 10 matrizes de {s2p} com telefone nesse estado."
            ir3 = base_ir({"kind": "list", "order": "cnpj", "limit": 10}, place_uf(short["uf"]), activity_sector(s2),
                extra_scope={"head_office": "yes", "contact": "phone", "exclude": {"municipalities": [short["id"]], "ufs": [], "sectors": [], "sector_cnaes": []}})
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "list", "ir": ir3, "act": "replace_slot"},
                ["SR-02", "SR-03", "SR-06", "SR-07", "SR-12"], geos[2], short["uf"], [], short["uf"] == "MT", short["uf"] != "MT",
                "dlg_p6_t3", "DLG/P6/T3_SECTOR_PIVOT_LIST", "supported_engine",
                "Dialogue turn 3: pivot to another sector and list head offices with phone while keeping state exclusion."))
        elif pattern == 7:
            homonyms = [
                ("Água Boa", 5170, "MT"),
                ("Canarana", 5171, "MT"),
                ("Várzea Grande", 5158, "MT"),
                ("Pedra Preta", 5165, "MT"),
                ("Rio Branco", 5072, "MT"),
                ("Santa Terezinha", 5173, "MT"),
                ("Nova Olímpia", 5520, "MT"),
                ("General Carneiro", 5113, "MT"),
                ("Novo Santo Antônio", 1088, "MT"),
            ]
            hname, hid, huf = homonyms[(unit_idx + d_idx) % len(homonyms)]
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {hname}?"
            turns.append(dialogue_item(0, t1, regs[0],
                {"decision": "clarify", "intent": None, "ir": None, "expected_behavior": f"Ask which UF is meant for {hname}."},
                ["SR-08"], geos[0], "UNRESOLVED", ["REG-MT-03"], True, False,
                "dlg_p7_t1", "DLG/P7/T1_HOMONYM_CLARIFY", "clarify_homonym",
                "Dialogue turn 1: homonymous municipality without UF triggers clarification."))
            t2 = f"Quantas {s1p} ativas existem na cidade de {hname}/{huf}?"
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(self.munis[hid]), activity_sector(s1))
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "new_query"},
                ["SR-02", "SR-06", "SR-08"], geos[1], huf, ["REG-MT-03"], True, False,
                "dlg_p7_t2", "DLG/P7/T2_RESOLVE_UF", "supported_aggregates",
                "Dialogue turn 2: user supplies MT UF to resolve the homonym."))
            t3 = f"Agora liste 20 matrizes de {s1p} com telefone na cidade de {hname}/{huf}."
            ir3 = base_ir({"kind": "list", "order": "cnpj", "limit": 20}, place_muni(self.munis[hid]), activity_sector(s1),
                extra_scope={"head_office": "yes", "contact": "phone"})
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "list", "ir": ir3, "act": "add_filter"},
                ["SR-02", "SR-03", "SR-06", "SR-08", "SR-12"], geos[2], huf, ["REG-MT-03"], True, False,
                "dlg_p7_t3", "DLG/P7/T3_LIST_HQ_PHONE", "supported_engine",
                "Dialogue turn 3: convert the resolved homonym into a head-office phone-qualified list."))
        else:
            t1 = f"{opener1}Quantas {s1p} ativas existem na cidade de {m1['name']}/{m1['uf']}?"
            ir1 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1))
            turns.append(dialogue_item(0, t1, regs[0], {"decision": "execute", "intent": "count", "ir": ir1, "act": "new_query"},
                ["SR-02", "SR-06", "SR-07"], geos[0], m1["uf"], reg1, mt1, ouf1, "dlg_p8_t1", "DLG/P8/T1_NEW",
                "supported_aggregates", "Dialogue turn 1: initial municipality count."))
            cap_reais = [30000, 50000, 100000][d_idx % 3]
            t2 = f"Agora só MEI com capital social de até {cap_reais} reais de {s1p} na cidade de {m1['name']}/{m1['uf']}."
            ir2 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m1), activity_sector(s1),
                extra_scope={"mei": "yes", "capital": {"from": None, "to": cap_reais * 100}})
            turns.append(dialogue_item(1, t2, regs[1], {"decision": "execute", "intent": "count", "ir": ir2, "act": "add_filter"},
                ["SR-02", "SR-03", "SR-06"], geos[1], m1["uf"], [], mt1, False, "dlg_p8_t2", "DLG/P8/T2_ADD_MEI_CAP_CEILING",
                "supported_engine", "Dialogue turn 2: add MEI and an upper capital-social bound."))
            t3 = f"Na verdade mantenha o MEI, tire o limite de capital e troque para {s1p} na cidade de {m2['name']}/{m2['uf']}."
            ir3 = base_ir({"kind": "count", "metric": "stock"}, place_muni(m2), activity_sector(s1), extra_scope={"mei": "yes"})
            turns.append(dialogue_item(2, t3, regs[2], {"decision": "execute", "intent": "count", "ir": ir3, "act": "compound_edit"},
                ["SR-02", "SR-03", "SR-06", "SR-07"], geos[2], m2["uf"], reg2, mt2, ouf2, "dlg_p8_t3", "DLG/P8/T3_KEEP_MEI_DROP_CAP",
                "supported_engine", "Dialogue turn 3: keep MEI, clear the capital ceiling and switch municipality."))
        return turns


def write_batch_files(batch_id: str, items: list[dict]):
    BATCHES_DIR.mkdir(parents=True, exist_ok=True)
    bpath = BATCHES_DIR / f"{batch_id}.jsonl"
    content = ("\n".join(json.dumps(item, ensure_ascii=False) for item in items) + "\n").encode("utf-8")
    bpath.write_bytes(content)
    fams = collections.Counter(x["meta"]["primary_family"] for x in items)
    geos = collections.Counter(x["meta"]["geographic_stratum"] for x in items)
    regs = collections.Counter(x["meta"]["register"] for x in items)
    splits = collections.Counter(x["meta"]["split"] for x in items)
    decs = collections.Counter(x["gold"]["decision"] for x in items)
    novelty = collections.Counter(x["meta"].get("novelty_reason") for x in items)
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
        "by_novelty_reason": {str(k): v for k, v in novelty.items()},
    }
    (BATCHES_DIR / f"{batch_id}-manifest.json").write_text(json.dumps(bman, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return bman


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--pilot-only", action="store_true")
    parser.add_argument("--all", action="store_true")
    args = parser.parse_args()

    author = Round13Author()
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
        b_items = author.build_unit(unit_idx, batch_id, unit_idx * 500 + 1, unit_idx * 20 + 1)
        batch_manifests.append(write_batch_files(batch_id, b_items))
        all_items.extend(b_items)
        if unit_idx % 20 == 0 or unit_idx == 99:
            print(f"Generated through {batch_id}: total items = {len(all_items)}")
    assert len(all_items) == 50000

    q_jsonl = ("\n".join(json.dumps(x, ensure_ascii=False) for x in all_items) + "\n").encode("utf-8")
    (EVAL_DIR / "questions.jsonl").write_bytes(q_jsonl)
    tsv_lines = ["id\tsplit\tprimary_family\tgeographic_stratum\tuf\tregister\tdecision\ttext"]
    for x in all_items:
        m = x["meta"]
        tsv_lines.append(f"{x['id']}\t{m['split']}\t{m['primary_family']}\t{m['geographic_stratum']}\t{m['uf']}\t{m['register']}\t{x['gold']['decision']}\t{x['text']}")
    tsv_bytes = ("\n".join(tsv_lines) + "\n").encode("utf-8")
    (EVAL_DIR / "questions.tsv").write_bytes(tsv_bytes)
    for sp in ("synthetic_dev", "synthetic_eval", "synthetic_challenge"):
        sp_items = [x for x in all_items if x["meta"]["split"] == sp]
        (EVAL_DIR / f"{sp}.jsonl").write_bytes(("\n".join(json.dumps(x, ensure_ascii=False) for x in sp_items) + "\n").encode("utf-8"))

    fams = collections.Counter(x["meta"]["primary_family"] for x in all_items)
    geos = collections.Counter(x["meta"]["geographic_stratum"] for x in all_items)
    regs = collections.Counter(x["meta"]["register"] for x in all_items)
    splits = collections.Counter(x["meta"]["split"] for x in all_items)
    decs = collections.Counter(x["gold"]["decision"] for x in all_items)
    novelty = collections.Counter(str(x["meta"].get("novelty_reason")) for x in all_items)
    mt_ctx_cnt = sum(1 for x in all_items if x["meta"]["mt_context"])
    ouf_reg_cnt = sum(1 for x in all_items if x["meta"]["other_uf_regional"])
    novel_cnt = sum(1 for x in all_items if x["meta"]["novelty_reason"] is not None)
    manifest = {
        "round": ROUND,
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
        "by_novelty_reason": dict(novelty),
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

    card = f"""# Dataset Card — Commercial Round 13 (`commercial-round-13`)

## Overview

- **Total items**: `50,000` pt-BR commercial questions and dialogue turns (`batch-001` 1,000-item pilot + `batch-002..batch-099` 98 batches of 500 items).
- **Questions JSONL SHA-256**: `{manifest['questions_jsonl_sha256']}`
- **Questions TSV SHA-256**: `{manifest['questions_tsv_sha256']}`
- **Data anchor**: `2026-10-11` (`Day(46305)`), current competence `2026-10` (`Competence(321)`).
- **Human validation**: `false` (deferred to product refinement).

## Quotas & Distributions

- **Geography (`50,000`)**: MT `30,000` across six strata covering every catalogued MT municipality; other 26 UFs `12,000` (floor 400 per UF); national/cross-UF/comparative `8,000`.
- **Primary business families (`50,000`)**: market size `5,000`; prospecting `7,000`; historical `5,000`; geography `5,000`; corporate `4,000`; supply chain `5,000`; cross-variable `5,000`; dialogue `6,000`; source quality `3,000`; unsupported `3,000`; tax/numeric `2,000`.
- **Registers (`50,000`)**: `conversational_professional` `35,000`; `formal_technical` `10,000`; `noisy_short` `5,000`.
- **Splits (`50,000`)**: `synthetic_dev` `35,000`; `synthetic_eval` `10,000`; `synthetic_challenge` `5,000`, partitioned by scenario/skeleton/session/CNPJ root.
- **Overlapping targets**: MT-context `{mt_ctx_cnt}`; other-UF regional `{ouf_reg_cnt}`; novel semantic constructions `{novel_cnt}`; clarify+reject `{decs['clarify'] + decs['reject']}`.
- **Novelty tags**: `{json.dumps(dict(novelty), ensure_ascii=False)}`.
"""
    (EVAL_DIR / "DATASET-CARD.md").write_text(card, encoding="utf-8")
    print("Completed full 50,000-item generation and manifests.")


if __name__ == "__main__":
    main()
