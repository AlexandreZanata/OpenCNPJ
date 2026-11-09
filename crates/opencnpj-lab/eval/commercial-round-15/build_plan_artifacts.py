#!/usr/bin/env python3
"""Build Milestone 1 source registry, regional glossary, gap inventory, and coverage matrix for Round 15."""

import collections
import glob
import hashlib
import json
from pathlib import Path
import tomllib

ROOT = Path(__file__).resolve().parents[4]
OUT_DIR = ROOT / "docs/intelligence/results/commercial-round-15"
CATALOG_PATH = ROOT / "crates/opencnpj-insights/catalog/municipalities.tsv"
SECTORS_PATH = ROOT / "crates/opencnpj-insights/catalog/sectors.toml"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def load_municipalities():
    munis = {}
    with CATALOG_PATH.open("r", encoding="utf-8") as f:
        for line in f.read().splitlines()[1:]:
            cols = line.split("\t")
            if not cols[0].isdigit():
                continue
            mid = int(cols[0])
            munis[mid] = {
                "id": mid,
                "tom": cols[1],
                "ibge": cols[2],
                "uf": cols[3],
                "name": cols[4],
                "folded": cols[5],
                "variants": cols[6],
                "pop_2022": int(cols[7] or 0),
                "pop_2026": int(cols[8] or 0),
                "reg_imediata": cols[13],
                "reg_intermediaria": cols[14],
                "capital": cols[15] == "1",
            }
    return munis


def classify_mt_stratum(m):
    reg_imed = m["reg_imediata"]
    name = m["name"]
    pantanal_names = {
        "Cáceres",
        "Poconé",
        "Santo Antônio de Leverger",
        "Nossa Senhora do Livramento",
        "Barão de Melgaço",
        "Porto Esperidião",
        "Curvelândia",
        "Lambari D'Oeste",
        "Rio Branco",
        "Salto do Céu",
        "Glória D'Oeste",
        "Porto Estrela",
    }
    if name in pantanal_names:
        return "mt_pantanal"
    if reg_imed == "510001":
        return "mt_metropolitan"
    if reg_imed in {"510007", "510008", "510010", "510011", "510012"} or name in {
        "Diamantino",
        "São José do Rio Claro",
        "Nova Maringá",
    }:
        return "mt_north_agro_industrial"
    if reg_imed in {"510016", "510017", "510018"}:
        return "mt_south_logistics_industrial"
    if reg_imed in {"510013", "510014", "510015"}:
        return "mt_east_araguaia_xingu"
    return "mt_west_parecis_guapore"


def main():
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    munis = load_municipalities()
    with SECTORS_PATH.open("rb") as f:
        sectors = tomllib.load(f)["sector"]

    mt_munis = {k: v for k, v in munis.items() if v["uf"] == "MT"}
    mt_by_stratum = collections.defaultdict(list)
    for m in sorted(mt_munis.values(), key=lambda x: x["id"]):
        stratum = classify_mt_stratum(m)
        mt_by_stratum[stratum].append({"id": m["id"], "ibge": m["ibge"], "name": m["name"], "reg_imediata": m["reg_imediata"]})

    assert sum(len(v) for v in mt_by_stratum.values()) == 142, "All 142 MT municipalities must be partitioned"

    # Audit rounds 01-08
    round_paths = sorted(glob.glob(str(ROOT / "crates/opencnpj-lab/eval/commercial-round-0*/questions.jsonl"))) + sorted(glob.glob(str(ROOT / "crates/opencnpj-lab/eval/commercial-round-10/questions.jsonl"))) + sorted(glob.glob(str(ROOT / "crates/opencnpj-lab/eval/commercial-round-11/questions.jsonl"))) + sorted(glob.glob(str(ROOT / "crates/opencnpj-lab/eval/commercial-round-12/questions.jsonl"))) + sorted(glob.glob(str(ROOT / "crates/opencnpj-lab/eval/commercial-round-13/questions.jsonl"))) + sorted(glob.glob(str(ROOT / "crates/opencnpj-lab/eval/commercial-round-14/questions.jsonl")))
    by_round = {}
    r01_08_categories = collections.Counter()
    r01_08_decisions = collections.Counter()
    r01_08_intents = collections.Counter()
    r01_08_acts = collections.Counter()
    r01_08_filter_combos = collections.Counter()
    r01_08_mt_munis = set()
    r01_08_all_munis = set()
    r01_08_ufs = collections.Counter()

    for rp in round_paths:
        p = Path(rp)
        rname = p.parent.name
        cnt = 0
        with p.open("r", encoding="utf-8") as f:
            for line in f:
                row = json.loads(line)
                cnt += 1
                r01_08_categories[row["meta"].get("category", "unknown")] += 1
                r01_08_decisions[row["gold"]["decision"]] += 1
                r01_08_intents[str(row["gold"].get("intent"))] += 1
                r01_08_acts[str(row["gold"].get("act"))] += 1
                ir = row["gold"].get("ir")
                if ir:
                    sc = ir.get("scope", {})
                    pl = sc.get("place")
                    if isinstance(pl, dict):
                        for mid in pl.get("municipalities", []):
                            r01_08_all_munis.add(mid)
                            if mid in mt_munis:
                                r01_08_mt_munis.add(mid)
                            if mid in munis:
                                r01_08_ufs[munis[mid]["uf"]] += 1
                    active = []
                    for k in [
                        "status",
                        "contact",
                        "mei",
                        "simples",
                        "porte",
                        "head_office",
                        "capital",
                        "natureza",
                        "within_ufs",
                        "place_population",
                        "exclude",
                        "opened",
                    ]:
                        v = sc.get(k)
                        if k == "status" and v == ["active"]:
                            continue
                        if v is not None and v != [] and v != {}:
                            active.append(k)
                    r01_08_filter_combos["+".join(sorted(active)) if active else "none"] += 1
        by_round[rname] = cnt

    source_registry = {
        "round": "commercial-round-15",
        "accessed_date": "2026-11-08",
        "pinned_internal_artifacts": {
            "snapshot_path": "/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj",
            "snapshot_sha256": "76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b",
            "current_aggregate_path": "/data/opencnpj-data/intelligence/agg/2026-10.ocagg",
            "history_aggregates_dir": "/data/opencnpj-data/intelligence/backfill/aggregates",
            "data_anchor": "2026-10-11",
            "data_anchor_day": 46305,
            "current_competence": "2026-10",
            "current_competence_id": 321,
            "available_competences_range": [298, 321],
            "municipalities_catalog_path": "crates/opencnpj-insights/catalog/municipalities.tsv",
            "municipalities_catalog_sha256": sha256_file(CATALOG_PATH),
            "municipalities_catalog_version": 202610,
            "municipalities_count": len(munis),
            "mt_municipalities_count": len(mt_munis),
            "sectors_catalog_path": "crates/opencnpj-insights/catalog/sectors.toml",
            "sectors_catalog_sha256": sha256_file(SECTORS_PATH),
            "sectors_catalog_version": 1,
            "sectors_count": len(sectors),
        },
        "regional_linguistic_and_cultural_sources": [
            {
                "source_id": "REG-MT-01",
                "title": "Tchá por Deus: uma análise semântico-lexical e cultural da expressão cuiabana",
                "publisher": "UNEMAT — Revista Estudos Acadêmicos de Letras (Reacl), v. 17, n. 01, 2024",
                "url": "https://periodicos.unemat.br/index.php/reacl/article/view/12250",
                "doi": "10.30681/real.v17i01.12250",
                "accessed_at": "2026-11-08",
                "verification_status": "HTTP_200_VERIFIED",
                "license": "CC BY 4.0",
                "locality": "Cuiabá and Baixada Cuiabana (Mato Grosso)",
                "role_in_round_15": "Primary academic linguistic reference for Cuiabano interjections and discourse markers ('Tchá por Deus', 'Vôte') and their pragmatic role in spoken and informal written pt-BR.",
                "reuse_policy": "Short lexical terms and synthetic commercial sentences inspired by documented usage; no verbatim reproduction of article prose.",
            },
            {
                "source_id": "REG-MT-02",
                "title": "MT Criativo — Tchá por Dios / Expressões e Cultura da Baixada Cuiabana",
                "publisher": "Secretaria de Estado de Cultura, Esporte e Lazer de Mato Grosso (SECEL-MT)",
                "url": "https://mtcriativo.mt.gov.br/index.php/directory/tcha-por-dios/",
                "accessed_at": "2026-11-08",
                "verification_status": "HTTP_200_VERIFIED",
                "license": "Public government cultural directory (State of Mato Grosso)",
                "locality": "Cuiabá / Várzea Grande / Mato Grosso",
                "role_in_round_15": "Public cultural registry documenting regional identity and spelling variants ('Tchá por Deus' / 'Tchá por Dios') in Mato Grosso creative economy contexts.",
                "reuse_policy": "Metadata and short regional lexical items only.",
            },
            {
                "source_id": "REG-MT-03",
                "title": "Danças, artes, comidas e costumes dão identidade a Mato Grosso (citing Antônio de Arruda 1998 & Antônio Loureiro 2006)",
                "publisher": "DETRAN-MT / SECOM-MT (Government of Mato Grosso)",
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "verification_status": "HTTP_200_VERIFIED",
                "license": "Public government communication citing published Mato Grosso lexicographical works",
                "locality": "Mato Grosso (Baixada Cuiabana, Pantanal, interior)",
                "role_in_round_15": "Documents traditional MT vocabulary and cultural practices ('vôte', 'tchá por Deus', 'é mato', 'pau-rodado', 'muxirum', 'quebra-torto') citing Antônio de Arruda ('O Linguajar Cuiabano e Outros Escritos', 1998) and Antônio Loureiro ('Cultura Mato-Grossense', 2006).",
                "reuse_policy": "Short lexical items and cultural context concepts used to ground synthetic pt-BR business questions.",
            },
            {
                "source_id": "REG-BR-01",
                "title": "Brazilian regional discourse markers and commercial colloquialisms across UFs",
                "publisher": "Standard Brazilian Portuguese dialectology and public regional usage (MG, RS, PE, BA, CE, PA, AM, GO, MS, PR, SC, SP, RJ)",
                "url": "https://www.academia.org.br/nossa-lingua/busca-no-vocabulario",
                "accessed_at": "2026-11-08",
                "verification_status": "DOCUMENTED_LEXICON",
                "license": "Public domain lexical items",
                "locality": "All 26 non-MT Brazilian states and DF",
                "role_in_round_15": "Provides documented regional interjections ('uai', 'sô', 'trem', 'bah', 'tchê', 'guri', 'oxe', 'oxente', 'visse', 'arretado', 'égua', 'piá', 'manezinho') for cross-UF minimal pairs.",
                "reuse_policy": "Single-word or two-word public regional interjections embedded in synthetic business queries.",
            },
        ],
        "external_socioeconomic_candidates_evaluated": [
            {
                "source_id": "EXT-IBGE-CEMPRE",
                "title": "IBGE — Cadastro Central de Empresas (CEMPRE)",
                "url": "https://www.ibge.gov.br/estatisticas/economicas/comercio/9016-cadastro-central-de-empresas.html",
                "sidra_tables": ["1685", "6449"],
                "accessed_at": "2026-11-08",
                "http_note": "Direct HTML URL returned HTTP 403 to automated non-browser client on 2026-11-08; metadata verified via IBGE/SIDRA public specification and docs/intelligence/ROUND-09-EXPANSION.md.",
                "license": "IBGE Open Data (public official statistics)",
                "granularity": "Municipality x CNAE Section/Division x Annual Reference Year",
                "join_keys": ["ibge_7digit_municipality_code", "cnae_section_or_division"],
                "integrated_in_runtime": False,
                "round_15_policy": "Not integrated into .ocnpj/.ocagg runtime artifacts. Questions asking for CEMPRE salaried headcount, total occupied personnel, or annual wage mass per municipality/sector, or attributing aggregate CEMPRE numbers to an individual CNPJ, must fail closed (reject) with a clear unavailable-data explanation.",
            },
            {
                "source_id": "EXT-IBGE-PAM",
                "title": "IBGE — Produção Agrícola Municipal (PAM)",
                "url": "https://www.ibge.gov.br/estatisticas/economicas/agricultura-e-pecuaria/9117-producao-agricola-municipal-culturas-temporarias-e-permanentes.html",
                "sidra_tables": ["5457"],
                "accessed_at": "2026-11-08",
                "license": "IBGE Open Data (public official statistics)",
                "granularity": "Municipality x Temporary/Permanent Crop (soy, corn, cotton, sugarcane, rice) x Annual Reference Year",
                "join_keys": ["ibge_7digit_municipality_code"],
                "integrated_in_runtime": False,
                "round_15_policy": "Not integrated into .ocnpj/.ocagg runtime artifacts. Questions asking for planted hectares, crop tonnage, harvest value, or statistical correlation between PAM crop output and CNPJ counts must fail closed (reject). Questions comparing cadastral counts or per-10k population density of grain storage, agricultural inputs, or freight transport across agro-industrial municipalities execute on the loaded RFB+IBGE-population artifacts.",
            },
            {
                "source_id": "EXT-IBGE-PPM",
                "title": "IBGE — Pesquisa da Pecuária Municipal (PPM)",
                "url": "https://www.ibge.gov.br/estatisticas/economicas/agricultura-e-pecuaria/9107-producao-da-pecuaria-municipal.html",
                "sidra_tables": ["3939"],
                "accessed_at": "2026-11-08",
                "license": "IBGE Open Data (public official statistics)",
                "granularity": "Municipality x Herd Type (cattle, swine, poultry, milk) x Annual Reference Year",
                "join_keys": ["ibge_7digit_municipality_code"],
                "integrated_in_runtime": False,
                "round_15_policy": "Not integrated into .ocnpj/.ocagg runtime artifacts. Questions asking for cattle herd headcount (cabeças de gado) or correlating PPM herd figures with slaughterhouse/veterinary CNPJs must fail closed (reject). Cadastral queries for slaughterhouses, cattle raising, milk, veterinary services, and refrigerated transport in Pantanal/Araguaia/Guaporé municipalities execute on the loaded artifacts.",
            },
        ],
    }

    regional_glossary = {
        "round": "commercial-round-15",
        "version": "1.0.0",
        "entries": [
            {
                "id": "reg_mt_vote",
                "term": "vôte",
                "variants": ["vôte", "vote"],
                "region_scope": "Mato Grosso (Baixada Cuiabana, Pantanal, statewide)",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-01", "REG-MT-03"],
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "meaning_en": "Traditional Mato Grosso interjection expressing surprise, astonishment, or emphasis.",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_intent",
                "semantic_rule": "When used as a conversational opener or interjection before/after an explicit cadastral question, ignore the interjection and execute the cadastral intent; if used alone without a cadastral question, reject.",
                "minimal_pair": {
                    "execute_example": "Vôte, quantas farmácias ativas existem na cidade de Cuiabá/MT?",
                    "reject_or_clarify_example": "Vôte, como tá o movimento hoje em Cuiabá?",
                },
                "reuse_rights": "Lexical item only; synthetic utterance.",
            },
            {
                "id": "reg_mt_tcha_por_deus",
                "term": "tchá por Deus",
                "variants": ["tchá por deus", "tcha por deus", "tchá por dios", "tcha por dios"],
                "region_scope": "Mato Grosso (Cuiabá, Várzea Grande, Baixada Cuiabana)",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-01", "REG-MT-02", "REG-MT-03"],
                "url": "https://periodicos.unemat.br/index.php/reacl/article/view/12250",
                "accessed_at": "2026-11-08",
                "meaning_en": "Cuiabano discourse marker of surprise, admiration, or emphasis.",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_intent",
                "semantic_rule": "Do not treat 'Deus'/'Dios' or 'Tchá' as an unknown place or sector when part of 'tchá por deus/dios'. Preserve the underlying cadastral query.",
                "minimal_pair": {
                    "execute_example": "Tchá por Deus, quantos estabelecimentos ativos de supermercado existem na cidade de Várzea Grande/MT?",
                    "reject_or_clarify_example": "Tchá por Deus, será que compensa abrir loja lá?",
                },
                "reuse_rights": "CC BY 4.0 lexical reference; synthetic utterance.",
            },
            {
                "id": "reg_mt_e_mato",
                "term": "é mato",
                "variants": ["é mato", "e mato"],
                "region_scope": "Mato Grosso and Central Brazil",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-03"],
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "meaning_en": "Predicate metaphor meaning 'is abundant / exists in large numbers'.",
                "uncertainty": "low",
                "nlu_behavior": "abundance_metaphor_clarify_unless_metric_explicit",
                "semantic_rule": "When a user asks whether a sector 'é mato' in a place without specifying an explicit cadastral metric, return clarify asking whether they want active count, density per 10k inhabitants, or a city comparison. When followed by an explicit count/density question, execute.",
                "minimal_pair": {
                    "execute_example": "Falam que oficina é mato por lá: quantos estabelecimentos ativos de oficina mecânica existem na cidade de Sorriso/MT?",
                    "reject_or_clarify_example": "Em Sorriso/MT oficina mecânica é mato mesmo?",
                },
                "reuse_rights": "Lexical item only; synthetic utterance.",
            },
            {
                "id": "reg_mt_pau_rodado",
                "term": "pau-rodado",
                "variants": ["pau-rodado", "pau rodado"],
                "region_scope": "Mato Grosso",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-03"],
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "meaning_en": "Person born outside Mato Grosso who migrated to and settled in the state.",
                "uncertainty": "low",
                "nlu_behavior": "context_preserve_or_partner_attribute_reject",
                "semantic_rule": "When used as biographical self-description by the speaker, ignore and execute the explicit cadastral query. When requested as a filter on company owners/partners ('empresas de sócio pau-rodado'), reject because partner birthplace/origin is not in public cadastral query scope.",
                "minimal_pair": {
                    "execute_example": "Cheguei como pau-rodado em Sinop/MT: quantos estabelecimentos ativos de contabilidade existem na cidade de Sinop/MT?",
                    "reject_or_clarify_example": "Quantas empresas em Sinop/MT pertencem a sócios pau-rodados?",
                },
                "reuse_rights": "Lexical item only; synthetic utterance.",
            },
            {
                "id": "reg_mt_quebra_torto",
                "term": "quebra-torto",
                "variants": ["quebra-torto", "quebra torto"],
                "region_scope": "Mato Grosso (Pantanal, Baixada Cuiabana, rural estates)",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-03"],
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "meaning_en": "Traditional early-morning reinforced meal in Mato Grosso and Pantanal.",
                "uncertainty": "low",
                "nlu_behavior": "ambiguous_activity_unless_sector_explicit",
                "semantic_rule": "If used as the sole activity description ('casas de quebra-torto'), return clarify because it maps to multiple food service CNAEs (padaria, restaurante, lanchonete). If the catalogue sector is explicitly named, execute.",
                "minimal_pair": {
                    "execute_example": "Pensando no café e quebra-torto em Poconé/MT: quantos estabelecimentos ativos de padaria existem na cidade de Poconé/MT?",
                    "reject_or_clarify_example": "Quantos lugares de quebra-torto existem em Poconé/MT?",
                },
                "reuse_rights": "Lexical item only; synthetic utterance.",
            },
            {
                "id": "reg_mt_muxirum",
                "term": "muxirum",
                "variants": ["muxirum", "muchirum"],
                "region_scope": "Mato Grosso (Baixada Cuiabana and interior)",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-03"],
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "meaning_en": "Collective work effort (mutirão) in Mato Grosso cultural tradition.",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_intent",
                "semantic_rule": "When used to describe a team prospecting effort before an explicit cadastral question, preserve the cadastral query.",
                "minimal_pair": {
                    "execute_example": "No muxirum da equipe comercial: quantos estabelecimentos ativos de loja de materiais de construção existem na cidade de Cáceres/MT?",
                    "reject_or_clarify_example": "Quem organizou o muxirum das empresas de Cáceres/MT?",
                },
                "reuse_rights": "Lexical item only; synthetic utterance.",
            },
            {
                "id": "reg_mt_informal_subregions",
                "term": "Baixada Cuiabana / Nortão / Vale do Araguaia",
                "variants": ["baixada cuiabana", "nortao", "nortão", "vale do araguaia", "medio norte", "médio norte"],
                "region_scope": "Mato Grosso subregions",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-03"],
                "url": "https://www.detran.mt.gov.br/web/mt/w/10050222-dancas-artes-comidas-e-costumes-dao-identidade-a-mato-grosso",
                "accessed_at": "2026-11-08",
                "meaning_en": "Regional designations within Mato Grosso that are not single municipalities or IBGE macro-regions.",
                "uncertainty": "medium",
                "nlu_behavior": "subregion_clarify_unless_municipalities_named",
                "semantic_rule": "When used as the sole geographic filter ('no Nortão', 'na Baixada Cuiabana') without naming specific municipalities or asking for the whole UF, return clarify asking which municipalities to include. When specific municipalities are named ('no Nortão, em Sinop/MT e Sorriso/MT'), execute on the named municipalities.",
                "minimal_pair": {
                    "execute_example": "Olhando o eixo do Nortão: compare Sorriso/MT e Sinop/MT em estabelecimentos ativos de armazém geral.",
                    "reject_or_clarify_example": "Quantas transportadoras ativas existem no Nortão?",
                },
                "reuse_rights": "Geographic vernacular; synthetic utterance.",
            },
            {
                "id": "reg_mg_uai_trem",
                "term": "uai / sô / trem",
                "variants": ["uai", "sô", "so", "trem"],
                "region_scope": "Minas Gerais and Goiás/Triângulo Mineiro",
                "uf_focus": "MG",
                "source_ids": ["REG-BR-01"],
                "url": "https://www.academia.org.br/nossa-lingua/busca-no-vocabulario",
                "accessed_at": "2026-11-08",
                "meaning_en": "Mineiro discourse markers ('uai', 'sô') and placeholder noun ('trem').",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_vs_placeholder_clarify",
                "semantic_rule": "'Uai' acts as a conversational discourse marker and preserves explicit cadastral queries; 'trem' replacing the activity name requires clarification of the CNAE sector.",
                "minimal_pair": {
                    "execute_example": "Uai, quantos estabelecimentos ativos de padaria existem na cidade de Uberlândia/MG?",
                    "reject_or_clarify_example": "Uai, quantas empresas daquele trem existem em Uberlândia/MG?",
                },
                "reuse_rights": "Public domain regionalism; synthetic utterance.",
            },
            {
                "id": "reg_rs_bah_tche",
                "term": "bah / tchê",
                "variants": ["bah", "tchê", "tche"],
                "region_scope": "Rio Grande do Sul and western Santa Catarina/Paraná",
                "uf_focus": "RS",
                "source_ids": ["REG-BR-01"],
                "url": "https://www.academia.org.br/nossa-lingua/busca-no-vocabulario",
                "accessed_at": "2026-11-08",
                "meaning_en": "Gaúcho discourse markers of emphasis or direct address.",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_intent",
                "semantic_rule": "'Bah' and 'tchê' do not alter the cadastral query when an explicit place and activity are provided.",
                "minimal_pair": {
                    "execute_example": "Bah, tchê, quantos estabelecimentos ativos de frigorífico existem na cidade de Passo Fundo/RS?",
                    "reject_or_clarify_example": "Bah, tchê, qual é o melhor frigorífico de Passo Fundo/RS?",
                },
                "reuse_rights": "Public domain regionalism; synthetic utterance.",
            },
            {
                "id": "reg_ne_oxe_oxente_visse",
                "term": "oxe / oxente / visse / arretado",
                "variants": ["oxe", "oxente", "visse", "arretado", "arretada"],
                "region_scope": "Northeast Brazil (PE, BA, CE, PB, RN, AL, SE, MA, PI)",
                "uf_focus": "PE",
                "source_ids": ["REG-BR-01"],
                "url": "https://www.academia.org.br/nossa-lingua/busca-no-vocabulario",
                "accessed_at": "2026-11-08",
                "meaning_en": "Northeastern discourse markers ('oxe', 'oxente', 'visse') and quality adjective ('arretado').",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_vs_quality_clarify",
                "semantic_rule": "'Oxe', 'oxente', and 'visse' preserve explicit cadastral queries; 'arretado/arretada' as a ranking criterion requires clarification of the cadastral metric.",
                "minimal_pair": {
                    "execute_example": "Oxe, quantos estabelecimentos ativos de farmácia existem na cidade de Recife/PE, visse?",
                    "reject_or_clarify_example": "Oxe, quais são as farmácias mais arretadas de Recife/PE?",
                },
                "reuse_rights": "Public domain regionalism; synthetic utterance.",
            },
            {
                "id": "reg_no_egua",
                "term": "égua",
                "variants": ["égua", "egua"],
                "region_scope": "North Brazil (PA, AM, AP, RR, RO, AC, TO)",
                "uf_focus": "PA",
                "source_ids": ["REG-BR-01"],
                "url": "https://www.academia.org.br/nossa-lingua/busca-no-vocabulario",
                "accessed_at": "2026-11-08",
                "meaning_en": "Northern discourse interjection of surprise ('égua!').",
                "uncertainty": "low",
                "nlu_behavior": "discourse_marker_preserve_intent",
                "semantic_rule": "When used as a sentence-initial interjection ('Égua, quantas...'), ignore and execute the explicit cadastral query.",
                "minimal_pair": {
                    "execute_example": "Égua, quantos estabelecimentos ativos de restaurante existem na cidade de Belém/PA?",
                    "reject_or_clarify_example": "Égua, onde vende o melhor açaí de Belém/PA?",
                },
                "reuse_rights": "Public domain regionalism; synthetic utterance.",
            },
            {
                "id": "biz_amb_revenda_agro",
                "term": "revenda agro / agronegócio / empresa do agro",
                "variants": ["revenda agro", "revendas agro", "agronegocio", "agronegócio", "empresa do agro", "empresas do agro"],
                "region_scope": "Mato Grosso and agricultural frontier UFs",
                "uf_focus": "MT",
                "source_ids": ["REG-MT-03", "REG-BR-01"],
                "url": "https://www.ibge.gov.br/estatisticas/economicas/agricultura-e-pecuaria/9117-producao-agricola-municipal-culturas-temporarias-e-permanentes.html",
                "accessed_at": "2026-11-08",
                "meaning_en": "Umbrella commercial terms spanning multiple distinct CNAEs (inputs, machinery, grain wholesale, storage, cattle, support services).",
                "uncertainty": "low",
                "nlu_behavior": "ambiguous_activity_clarify",
                "semantic_rule": "Unless disambiguated by a concrete catalogue sector or explicit CNAE code, return clarify asking which specific activity is meant.",
                "minimal_pair": {
                    "execute_example": "Quantos estabelecimentos ativos de loja de insumos agrícolas existem na cidade de Sorriso/MT?",
                    "reject_or_clarify_example": "Quantas revendas agro ativas existem na cidade de Sorriso/MT?",
                },
                "reuse_rights": "Commercial domain terminology; synthetic utterance.",
            },
            {
                "id": "biz_amb_saturation",
                "term": "cabe mais uma / tá saturado / tem espaço pra mais",
                "variants": ["cabe mais uma", "cabe mais um", "ta saturado", "tá saturado", "mercado saturado", "tem espaco pra mais", "tem espaço pra mais"],
                "region_scope": "National commercial prospecting",
                "uf_focus": "ALL",
                "source_ids": ["REG-BR-01"],
                "url": "https://www.academia.org.br/nossa-lingua/busca-no-vocabulario",
                "accessed_at": "2026-11-08",
                "meaning_en": "Commercial saturation judgment without a specified cadastral threshold.",
                "uncertainty": "low",
                "nlu_behavior": "ambiguous_quality_clarify",
                "semantic_rule": "When a user asks whether a local market is saturated or has room for one more establishment without specifying a metric, return clarify asking whether to check total active establishments, density per 10k inhabitants, or inhabitants per establishment.",
                "minimal_pair": {
                    "execute_example": "Qual a densidade por 10 mil habitantes de farmácia em Sinop/MT?",
                    "reject_or_clarify_example": "O mercado de farmácia em Sinop/MT já tá saturado ou cabe mais uma?",
                },
                "reuse_rights": "Commercial domain terminology; synthetic utterance.",
            },
        ],
    }

    gap_inventory = {
        "round": "commercial-round-15",
        "prior_rounds_audited": by_round,
        "prior_rounds_total_items": sum(by_round.values()),
        "prior_rounds_distributions": {
            "categories": dict(r01_08_categories),
            "decisions": dict(r01_08_decisions),
            "intents": dict(r01_08_intents),
            "acts": dict(r01_08_acts),
            "filter_combinations": dict(r01_08_filter_combos),
            "distinct_municipalities_used": len(r01_08_all_munis),
            "mt_municipalities_used": len(r01_08_mt_munis),
            "mt_municipalities_missing": 142 - len(r01_08_mt_munis),
        },
        "identified_gaps_and_round_15_remediation": [
            {
                "gap_id": "GAP-15-01-SECTOR-COLLOQUIALISMS",
                "description": "Round 14 added pintor, vidraceiro and costureira; funileiro, lanterneiro, confeiteiro, doceiro and jardineiro were still absent.",
                "round_15_target": "Add those documented commercial surfaces to existing sectors without inventing customer speech.",
            },
            {
                "gap_id": "GAP-15-02-THREE-CITY-DENSITY",
                "description": "Round 14 added two-city per-10k density; three-city density was not covered.",
                "round_15_target": "Alternate two- and three-municipality density comparisons.",
            },
            {
                "gap_id": "GAP-15-03-CAPITAL-RANGE-COMBINATIONS",
                "description": "Round 14 covered capital ceilings and zero capital, but bounded capital intervals combined with tax regime or MEI lists were still sparse.",
                "round_15_target": "Add Simples/non-MEI and MEI list variants inside bounded capital intervals.",
            },
            {
                "gap_id": "GAP-15-04-CNAE-GROUP",
                "description": "Round 14 added CNAE division concentration; the group hierarchy was still uncovered.",
                "round_15_target": "Add a CNAE-group concentration breakdown.",
            },
        ],
    }

    non_mt_ufs = [
        "AC", "AL", "AM", "AP", "BA", "CE", "DF", "ES", "GO", "MA",
        "MG", "MS", "PA", "PB", "PE", "PI", "PR", "RJ", "RN", "RO",
        "RR", "RS", "SC", "SE", "SP", "TO",
    ]
    # 26 UFs * 400 = 10,400 + 1,600 novelty allocation = 12,000
    uf_quotas = {uf: 400 for uf in non_mt_ufs}
    novelty_uf_bonuses = {
        "MG": 120, "RS": 120, "PE": 100, "BA": 100, "GO": 120, "MS": 120,
        "PR": 100, "SC": 80, "SP": 120, "PA": 100, "CE": 80, "AM": 60,
        "RJ": 60, "TO": 60, "RO": 60, "MA": 40, "PI": 30, "PB": 30,
        "RN": 20, "AL": 20, "SE": 15, "ES": 15, "DF": 15, "AC": 5, "AP": 5, "RR": 5,
    }
    assert sum(novelty_uf_bonuses.values()) == 1600
    for uf, bonus in novelty_uf_bonuses.items():
        uf_quotas[uf] += bonus
    assert sum(uf_quotas.values()) == 12000

    coverage_matrix = {
        "round": "commercial-round-15",
        "total_items": 50000,
        "batch_plan": {
            "total_batches": 99,
            "pilot_batch": {"batch_id": "batch-001", "items": 1000, "id_range": ["commercial-r15-00001", "commercial-r15-01000"]},
            "regular_batches": {
                "batch_id_range": ["batch-002", "batch-099"],
                "count": 98,
                "items_per_batch": 500,
                "total_items": 49000,
                "id_range": ["commercial-r15-01001", "commercial-r15-50000"],
            },
        },
        "primary_business_families": {
            "market_size_density_competition": {"total": 5000, "pilot": 100, "per_500_batch": 50},
            "prospecting_qualification_territory": {"total": 7000, "pilot": 140, "per_500_batch": 70},
            "historical_stock_flows_seasonality": {"total": 5000, "pilot": 100, "per_500_batch": 50},
            "geography_catchments_comparisons": {"total": 5000, "pilot": 100, "per_500_batch": 50},
            "corporate_structure_hq_branches_identity": {"total": 4000, "pilot": 80, "per_500_batch": 40},
            "supply_chain_complementary": {"total": 5000, "pilot": 100, "per_500_batch": 50},
            "cross_variable_correlation": {"total": 5000, "pilot": 100, "per_500_batch": 50},
            "dialogue_corrections_references_planning": {"total": 6000, "pilot": 120, "per_500_batch": 60},
            "source_quality_provenance_uncertainty": {"total": 3000, "pilot": 60, "per_500_batch": 30},
            "unsupported_missing_contradictions": {"total": 3000, "pilot": 60, "per_500_batch": 30},
            "tax_size_legal_capital_numeric": {"total": 2000, "pilot": 40, "per_500_batch": 20},
        },
        "orthogonal_geography": {
            "mato_grosso_total": 30000,
            "mato_grosso_strata": {
                "mt_metropolitan": {"total": 5000, "pilot": 100, "per_500_batch": 50, "municipalities": mt_by_stratum["mt_metropolitan"]},
                "mt_north_agro_industrial": {"total": 6000, "pilot": 120, "per_500_batch": 60, "municipalities": mt_by_stratum["mt_north_agro_industrial"]},
                "mt_south_logistics_industrial": {"total": 5000, "pilot": 100, "per_500_batch": 50, "municipalities": mt_by_stratum["mt_south_logistics_industrial"]},
                "mt_east_araguaia_xingu": {"total": 4500, "pilot": 90, "per_500_batch": 45, "municipalities": mt_by_stratum["mt_east_araguaia_xingu"]},
                "mt_west_parecis_guapore": {"total": 4500, "pilot": 90, "per_500_batch": 45, "municipalities": mt_by_stratum["mt_west_parecis_guapore"]},
                "mt_pantanal": {"total": 5000, "pilot": 100, "per_500_batch": 50, "municipalities": mt_by_stratum["mt_pantanal"]},
            },
            "other_26_ufs_total": 12000,
            "other_26_ufs_floor_per_uf": 400,
            "other_26_ufs_allocation": uf_quotas,
            "national_cross_uf_comparative_unresolved_total": 8000,
        },
        "exclusive_registers": {
            "conversational_professional": {"total": 35000, "pilot": 700, "per_500_batch": 350},
            "formal_technical": {"total": 10000, "pilot": 200, "per_500_batch": 100},
            "noisy_short": {"total": 5000, "pilot": 100, "per_500_batch": 50},
        },
        "exclusive_splits": {
            "synthetic_dev": {"total": 35000, "pilot": 700, "per_500_batch": 350},
            "synthetic_eval": {"total": 10000, "pilot": 200, "per_500_batch": 100},
            "synthetic_challenge": {"total": 5000, "pilot": 100, "per_500_batch": 50},
            "partition_rule": "Split by disjoint scenario_family_id, surface skeleton, CNPJ roots, and complete dialogue session_id; zero cross-split template or session leakage.",
        },
        "overlapping_floors": {
            "mt_context_regional_economic_min": 8000,
            "other_uf_regional_min": 4000,
            "novel_semantic_constructions_min": 15000,
            "clarify_or_reject_outcomes_min": 5000,
        },
    }

    (OUT_DIR / "source-registry.json").write_text(json.dumps(source_registry, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (OUT_DIR / "regional-glossary.json").write_text(json.dumps(regional_glossary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (OUT_DIR / "prior-rounds-gap-inventory.json").write_text(json.dumps(gap_inventory, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    (OUT_DIR / "coverage-matrix.json").write_text(json.dumps(coverage_matrix, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    plan_md = f"""# Commercial Round 15 — Source Registry, Gap Inventory, and Coverage Plan

## 1. Scope and Pinned Data Identity

- **Target size**: `50,000` pt-BR commercial questions/turns (`batch-001` pilot of `1,000` + `98` batches of `500`).
- **Snapshot**: `/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj` (`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`).
- **Current aggregate**: `/data/opencnpj-data/intelligence/agg/2026-10.ocagg` (competence `2026-10` / `321`, anchor `2026-10-11` / `Day(46305)`).
- **Historical aggregates**: `/data/opencnpj-data/intelligence/backfill/aggregates` (`2024-11` through `2026-10`, competences `298..=321`).
- **Catalogues**: `municipalities.tsv` (`{len(munis)}` municipalities, `142` in Mato Grosso, version `202610`) and `sectors.toml` (`{len(sectors)}` curated commercial sectors, version `1`).

## 2. Verified Regional Sources & External Socioeconomic Candidates

- **Mato Grosso linguistic & cultural sources** (verified `2026-11-08`):
  - `REG-MT-01`: UNEMAT *Revista Estudos Acadêmicos de Letras* (v. 17, n. 01, 2024), *"Tchá por Deus: uma análise semântico-lexical e cultural da expressão cuiabana"*, DOI `10.30681/real.v17i01.12250` (CC BY 4.0).
  - `REG-MT-02`: SECEL-MT / MT Criativo cultural directory (`https://mtcriativo.mt.gov.br/index.php/directory/tcha-por-dios/`).
  - `REG-MT-03`: DETRAN-MT / SECOM-MT cultural identity article citing Antônio de Arruda (*O Linguajar Cuiabano e Outros Escritos*, 1998) and Antônio Loureiro (*Cultura Mato-Grossense*, 2006).
- **External socioeconomic candidates evaluated** (`source-registry.json`):
  - `EXT-IBGE-CEMPRE` (IBGE Cadastro Central de Empresas), `EXT-IBGE-PAM` (IBGE Produção Agrícola Municipal), and `EXT-IBGE-PPM` (IBGE Pesquisa da Pecuária Municipal) are documented with join keys (`ibge` 7-digit municipality code, CNAE section/division) and granularity. None of the three is integrated into the runtime `.ocnpj`/`.ocagg` artifacts; questions requesting CEMPRE staff/wages, PAM planted area/tonnage, PPM herd counts, or statistical correlation coefficients ($r$, $R^2$, $p$-value) fail closed (`reject`) rather than fabricating values.

## 3. Prior-Round Gap Inventory (Rounds 01–09)

- Rounds 01–09 contain `159,000` questions/turns. Round 15 keeps the same quotas and attacks the remaining Round 09 gaps:
  - Remaining sector colloquialisms (funileiro, lanterneiro, confeiteiro, doceiro, jardineiro).
  - Three-city density comparisons and CNAE-group concentration.
  - Bounded capital intervals combined with tax regime or MEI lists.
  - Frozen conversational suites and the UF-ranking differential rows.

## 4. Orthogonal Quota Allocation (`50,000` items)

| Dimension | Allocation |
|---|---|
| **Geography** | `30,000` MT (`5,000` metropolitan, `6,000` north agro-industrial, `5,000` south logistics/industrial, `4,500` east Araguaia-Xingu, `4,500` west Parecis-Guaporé, `5,000` Pantanal; all `142` MT municipalities covered) + `12,000` across other 26 UFs (`>= 400` per UF) + `8,000` national/cross-UF/comparative/unresolved |
| **11 Primary Business Families** | `market_size_density_competition` (`5,000`), `prospecting_qualification_territory` (`7,000`), `historical_stock_flows_seasonality` (`5,000`), `geography_catchments_comparisons` (`5,000`), `corporate_structure_hq_branches_identity` (`4,000`), `supply_chain_complementary` (`5,000`), `cross_variable_correlation` (`5,000`), `dialogue_corrections_references_planning` (`6,000`), `source_quality_provenance_uncertainty` (`3,000`), `unsupported_missing_contradictions` (`3,000`), `tax_size_legal_capital_numeric` (`2,000`) |
| **Exclusive Registers** | `conversational_professional` (`35,000`), `formal_technical` (`10,000`), `noisy_short` (`5,000`) |
| **Exclusive Splits** | `synthetic_dev` (`35,000`), `synthetic_eval` (`10,000`), `synthetic_challenge` (`5,000`) partitioned by disjoint `scenario_family_id`, surface skeleton, CNPJ roots, and `session_id` |
| **Overlapping Floors** | `>= 8,000` MT-context, `>= 4,000` other-UF regional, `>= 15,000` novel semantic constructions, `>= 5,000` `clarify`/`reject` |
"""
    (OUT_DIR / "PLAN.md").write_text(plan_md, encoding="utf-8")
    print("Generated Milestone 1 artifacts in", OUT_DIR)


if __name__ == "__main__":
    main()
