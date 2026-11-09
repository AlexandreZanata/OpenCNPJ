# Commercial Round 11 — Source Registry, Gap Inventory, and Coverage Plan

## 1. Scope and Pinned Data Identity

- **Target size**: `50,000` pt-BR commercial questions/turns (`batch-001` pilot of `1,000` + `98` batches of `500`).
- **Snapshot**: `/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj` (`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`).
- **Current aggregate**: `/data/opencnpj-data/intelligence/agg/2026-10.ocagg` (competence `2026-10` / `321`, anchor `2026-10-11` / `Day(46305)`).
- **Historical aggregates**: `/data/opencnpj-data/intelligence/backfill/aggregates` (`2024-11` through `2026-10`, competences `298..=321`).
- **Catalogues**: `municipalities.tsv` (`5572` municipalities, `142` in Mato Grosso, version `202610`) and `sectors.toml` (`214` curated commercial sectors, version `1`).

## 2. Verified Regional Sources & External Socioeconomic Candidates

- **Mato Grosso linguistic & cultural sources** (verified `2026-11-08`):
  - `REG-MT-01`: UNEMAT *Revista Estudos Acadêmicos de Letras* (v. 17, n. 01, 2024), *"Tchá por Deus: uma análise semântico-lexical e cultural da expressão cuiabana"*, DOI `10.30681/real.v17i01.12250` (CC BY 4.0).
  - `REG-MT-02`: SECEL-MT / MT Criativo cultural directory (`https://mtcriativo.mt.gov.br/index.php/directory/tcha-por-dios/`).
  - `REG-MT-03`: DETRAN-MT / SECOM-MT cultural identity article citing Antônio de Arruda (*O Linguajar Cuiabano e Outros Escritos*, 1998) and Antônio Loureiro (*Cultura Mato-Grossense*, 2006).
- **External socioeconomic candidates evaluated** (`source-registry.json`):
  - `EXT-IBGE-CEMPRE` (IBGE Cadastro Central de Empresas), `EXT-IBGE-PAM` (IBGE Produção Agrícola Municipal), and `EXT-IBGE-PPM` (IBGE Pesquisa da Pecuária Municipal) are documented with join keys (`ibge` 7-digit municipality code, CNAE section/division) and granularity. None of the three is integrated into the runtime `.ocnpj`/`.ocagg` artifacts; questions requesting CEMPRE staff/wages, PAM planted area/tonnage, PPM herd counts, or statistical correlation coefficients ($r$, $R^2$, $p$-value) fail closed (`reject`) rather than fabricating values.

## 3. Prior-Round Gap Inventory (Rounds 01–09)

- Rounds 01–09 contain `159,000` questions/turns. Round 11 keeps the same quotas and attacks the remaining Round 09 gaps:
  - Remaining sector colloquialisms (churrascaria, hamburgueria, cafeteria, sorveteria, adega).
  - Follow-up act taxonomy (compound keep/drop edits).
  - Frozen conversational suites (adv, h-informal, dialogues) and implicit referents.
  - The two frozen UF-ranking differential rows.

## 4. Orthogonal Quota Allocation (`50,000` items)

| Dimension | Allocation |
|---|---|
| **Geography** | `30,000` MT (`5,000` metropolitan, `6,000` north agro-industrial, `5,000` south logistics/industrial, `4,500` east Araguaia-Xingu, `4,500` west Parecis-Guaporé, `5,000` Pantanal; all `142` MT municipalities covered) + `12,000` across other 26 UFs (`>= 400` per UF) + `8,000` national/cross-UF/comparative/unresolved |
| **11 Primary Business Families** | `market_size_density_competition` (`5,000`), `prospecting_qualification_territory` (`7,000`), `historical_stock_flows_seasonality` (`5,000`), `geography_catchments_comparisons` (`5,000`), `corporate_structure_hq_branches_identity` (`4,000`), `supply_chain_complementary` (`5,000`), `cross_variable_correlation` (`5,000`), `dialogue_corrections_references_planning` (`6,000`), `source_quality_provenance_uncertainty` (`3,000`), `unsupported_missing_contradictions` (`3,000`), `tax_size_legal_capital_numeric` (`2,000`) |
| **Exclusive Registers** | `conversational_professional` (`35,000`), `formal_technical` (`10,000`), `noisy_short` (`5,000`) |
| **Exclusive Splits** | `synthetic_dev` (`35,000`), `synthetic_eval` (`10,000`), `synthetic_challenge` (`5,000`) partitioned by disjoint `scenario_family_id`, surface skeleton, CNPJ roots, and `session_id` |
| **Overlapping Floors** | `>= 8,000` MT-context, `>= 4,000` other-UF regional, `>= 15,000` novel semantic constructions, `>= 5,000` `clarify`/`reject` |
