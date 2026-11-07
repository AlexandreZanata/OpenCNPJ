# 09 — Data assets: catalogues, external data and aggregates

Everything the assistant needs besides the `.ocnpj` snapshot. All assets are derived,
versioned, checksummed and rebuildable from recorded sources. Facts marked [VERIFIED]
were checked on 2026-11-04 (web research, see §8); [MEASURED] come from
[results/2026-11-04-aggregate-cardinality.md](results/2026-11-04-aggregate-cardinality.md).

## 1. Inventory

| Asset | Source | Format / size | Refresh | Notes |
|---|---|---|---|---|
| RFB municipalities, CNAEs, naturezas, motivos | Inside the snapshot (`reftables` section) | 5,572 / 1,359 / 92 / 63 rows [MEASURED] | Monthly | Names uppercase without accents; municipality table has no UF |
| TOM/SIAFI ↔ IBGE code table | Tesouro Transparente "Lista de municípios do SIAFI": `https://www.tesourotransparente.gov.br/ckan/dataset/abb968cb-3710-4f85-89cf-875c91b9c7f6/resource/eebb3bc6-9eea-4496-8bcf-304f33155282/download/tabmun.csv` [VERIFIED] | `;`, no header, 5,589 rows: SIAFI code, municipality CNPJ, name, UF, IBGE code; last modified 2021-12-21; CKAN license "not specified" | Rare | Matches every RFB 2026-10 code except `1182` BOA ESPERANCA DO NORTE (new municipality, IBGE 5101837) and `9707` EXTERIOR [VERIFIED] → a reviewed overrides file covers them |
| Municipalities with accented names, UF, regions | IBGE Localidades API `https://servicodados.ibge.gov.br/api/v1/localidades/municipios` [VERIFIED] | JSON, 5,571 entries, micro/mesorregião and região imediata/intermediária | Yearly | Display names and region hierarchy |
| Population, census | IBGE SIDRA agregado 4709, variável 93, period 2022 [VERIFIED]; e.g. `https://servicodados.ibge.gov.br/api/v3/agregados/4709/periodos/2022/variaveis/93?localidades=N6[all]` | JSON, 5,570 municipalities | Decennial | São Paulo 11,451,999 |
| Population, annual estimate | IBGE SIDRA agregado 6579, variável 9324, latest period 2026 [VERIFIED] | JSON, 5,571 municipalities | Yearly | Default denominator for "por habitante" (year printed in answers) |
| Neighbouring municipalities | IBGE "Municípios limítrofes" 2024: `https://geoftp.ibge.gov.br/organizacao_do_territorio/estrutura_territorial/municipios_limitrofes/2024/BR_Municipios_2024_LIMITROFES.ods` [VERIFIED] | ODS 1.1 MB, ~33.4k pairs (CD_MUN, CD_LIM, shared border km) | Yearly | "Cidades vizinhas" without computing geometry |
| Municipal seat coordinates | IBGE Localidades do Brasil 2022 GeoPackage (6.2 MB), filter `SCT_LOCALIDADE='Sede Municipal'` [VERIFIED] | GeoPackage = SQLite (readable with the workspace's `rusqlite`) | Rare | Distances for "num raio de 100 km" |
| CNAE 2.3 hierarchy and activity index | IBGE CNAE API `https://servicodados.ibge.gov.br/api/v2/cnae/subclasses` [VERIFIED]: 1,332 subclasses with section/division/group/class, 17,180 activity phrases, "compreende / não compreende" notes; CONCLA spreadsheets as backup | JSON | Rare | RFB lists 1,359 codes vs 1,332 in CNAE 2.3 → legacy codes need the CONCLA correspondence tables |
| Cross-check dataset | `kelvins/municipios-brasileiros` (MIT) [VERIFIED]: IBGE code, name, lat/lon, `siafi_id`, DDD | CSV | — | Used only to cross-check the official joins, never as the source of truth |
| Out-of-domain negatives (eval/training only) | `OHF-Voice/intents`, `sentences/pt-BR` (CC BY 4.0) [VERIFIED] | YAML | — | Not shipped in the product; attribution in the dataset card |
| Historical RFB releases | RFB Nextcloud share: every month **2023-05 … 2026-10** (42 releases) [VERIFIED], nothing older; retention policy not published [could not verify] | 37 ZIPs ≈ 7.8 GB per month | Monthly | Listed under `Dados/Cadastros/CNPJ/` of share token `gn672Ad4CF8N6TK`; `opencnpj download` defaults to another token: WP-1.5 checks whether `--month` reaches old months, else uses the existing `RFB_BASE_URL` / `RFB_SHARE_TOKEN` overrides (no code change) |

Licences: RFB CNPJ open data is published as "Creative Commons Attribution" on
dados.gov.br [VERIFIED]; IBGE terms of use were not checked (WP-1.2 records them);
Tesouro's table has no stated licence (keep provenance and attribution).

## 2. Municipality catalogue

`catalog/municipalities.tsv` (generated, < 1 MB):

| Column | Meaning |
|---|---|
| `id` | dense id 0..N (u16), stable within a catalogue version |
| `tom` | RFB/SIAFI 4-digit code (join key to the snapshot) |
| `ibge` | 7-digit IBGE code (join key to population, geography) |
| `uf` | from the official table, **never inferred from data** (codes 6969 and 0403 have rows in another UF in the 2026-10 data [MEASURED]) |
| `name` | IBGE display name with accents ("Cuiabá") |
| `folded`, `variants` | matching forms (05-INTERPRETER.md §5), curated aliases merged in |
| `pop_census_2022`, `pop_est`, `pop_est_year` | population |
| `lat`, `lon` | seat coordinates |
| `neighbours` | list of `id`s (from the IBGE adjacency table) |
| `reg_imediata`, `reg_intermediaria`, `capital` | region ids, state capital flag |

Build checks (fail the build): every TOM code seen in the snapshot is mapped or listed in
`overrides.tsv` with a source note; every IBGE code has population; name mismatches
between RFB and IBGE beyond accent/case folding are reported for review.

## 3. Sector catalogue (curated, versioned)

The catalogue is the product's most important hand-maintained data. It maps the words
people use to explicit CNAE sets (SR-07).

```toml
[[sector]]
id = "bakery"
display = { sing = "padaria", plural = "padarias", gender = "f" }
aliases = ["padaria", "panificadora", "panificação", "padoca"]
cnae_core = ["4721101", "4721102"]          # what "padaria" counts by default
cnae_extended = ["1091101", "1091102"]      # offered: "incluir fabricação de pães"
scope = "main"                               # main CNAE unless the user asks for secondary
notes = "Revenda e produção própria; fabricação industrial fica no conjunto estendido."
since = "v1"
```

Build process (WP-1.3):

1. Import the CNAE hierarchy and the 17,180 activity phrases from the IBGE API.
2. Generate candidate aliases per subclass from the official description and activity
   phrases (head nouns, plural forms), marked `auto`.
3. Curate ~200 popular sectors by hand (padaria, farmácia/drogaria, supermercado/mercado,
   oficina mecânica, salão de beleza, barbearia, academia, restaurante, lanchonete, bar,
   pet shop, clínica veterinária, construção, material de construção, transporte de carga,
   transporte de passageiros, tecnologia da informação, contabilidade, advocacia,
   imobiliária, hotel, escola, creche, posto de combustível, …), each with core and
   extended sets.
4. Reports: aliases that map to more than one sector (→ clarification options), subclasses
   not reachable by any alias, legacy RFB codes without a CNAE 2.3 equivalent.

Every answer that uses a sector lists its codes; the catalogue version is part of the IR
and of cache keys.

## 4. Aggregates artifact (`.ocagg`, draft 0.1)

A separate, immutable, memory-mapped file per competence, built from a **verified**
`.ocnpj` snapshot. It is not a new `.ocnpj` format version and does not change
FORMAT.md; it gets its own spec (`docs/intelligence/AGGREGATES-FORMAT.md`, written in
WP-1.4) with the same conventions: little-endian, header + sections + section table +
footer, xxh3 checksums, SHA-256 sidecar, manifest with the source snapshot SHA-256.

Tables (dimension ids are catalogue ids; counts are establishments):

| Table | Key | Cells (2026-10) | Answers |
|---|---|---|---|
| T1 active stock | (CNAE, município, matriz/filial, porte, MEI, Simples) | ≤ 2.83 M without Simples [MEASURED]; Simples adds at most 2× | Counts, comparisons, rankings, densities, breakdowns by porte/MEI/matriz |
| T2 stock by status | (CNAE, município, situação, matriz/filial) | 4.57 M [MEASURED] | "inaptas", "suspensas", all-status counts |
| T3 registrations | (CNAE, município, month of `data_inicio_atividade`), monthly from 2021 + yearly before | 8.7 M monthly since 2021 + yearly (≤ 13.1 M for all years) [MEASURED] | "abriram em 2025", trends of registrations |
| T4 baixas | (CNAE, município, month of `data_situacao_cadastral`) for situação 08, monthly from 2021 | 5.4 M [MEASURED] | "fecharam no ano passado", trends of closures |
| T5 secondary activity | (CNAE, município) active establishments having it as secondary | 2.79 M [MEASURED] | "incluindo atividade secundária" counts (not rankings of overlaps) |
| M* marginals | totals per (CNAE), (município), (UF, CNAE), (UF) for T1–T4 | thousands | National and UF answers without scanning |

Layout: cells sorted by key; columnar blocks of 4,096 cells (delta-coded keys, bit-packed
counts, optional zstd per block); a block directory with first keys for binary search;
a per-CNAE offset table (sector-first queries: "farmácias em todas as cidades de MT") and
marginals for place-first queries ("quantas empresas tem em Sorriso"). Size target per
competence 30–60 MB on disk for T1–T5 **[ESTIMATE]** (8 B/cell raw before compression).

History: only T1/T2 are kept per past competence (stock change, SR-05); T3/T4 come from
the latest snapshot (registration dates do not change between releases; baixas are
counted by their current status date, C-CLOSE).

Build: one pass over the snapshot's documents using doc values (município, main CNAE),
the postings bitmaps of situação/porte/MEI/Simples/matriz-filial, and the numeric columns
for dates; parallel by document range (rayon). Target < 5 min on the build host
**[HYPOTHESIS]**. Verification: marginals compared with the engine's exact counts for
every CNAE and every UF, plus 10,000 seeded random cells (NFR-12).

## 5. Historical backfill (WP-1.5)

For each competence to keep (D-04: 24 months, 2024-11 … 2026-10; up to 42 available):
download → `opencnpj build` → `verify` → build T1/T2 → delete the ZIPs and the snapshot.
Peak disk ≈ 7.8 GB ZIPs + 18 GB build scratch + 8.3 GB snapshot ≈ 35 GB on `/data` (109 GB
free) [ESTIMATE from BENCHMARKS.md build figures]. One competence at a time, as a
background job at idle priority; each step logged.

## 6. Repository policy

* Committed: catalogue **sources** that are hand-edited (sector catalogue, aliases,
  overrides, cue lexicons, templates) and small generated catalogues (< 2 MB each, TSV)
  with a manifest of source URLs and SHA-256s.
* Not committed: raw downloads, `.ocagg` files, synthetic datasets, classifier weights
  (published as build/release artifacts, referenced by SHA-256 in a manifest).

## 7. Refresh

| Trigger | Rebuild |
|---|---|
| New RFB release (monthly, existing auto-update) | T1–T5 for the new competence; catalogue check for new TOM codes / CNAE codes |
| New IBGE estimate (yearly) | Municipality catalogue (population columns), new catalogue version |
| Catalogue edit | Language pack + catalogue version; aggregates unchanged (they are keyed by CNAE, not sector) |

## 8. Sources consulted (2026-11-04)

Tesouro Transparente SIAFI municipality list; IBGE SIDRA API (agregados 4709, 6579);
IBGE Localidades API; IBGE geoftp (municípios limítrofes 2024, Localidades do Brasil 2022,
malha municipal 2025); IBGE CNAE API v2 and CONCLA download page; dados.gov.br CNPJ
dataset page; RFB Nextcloud share listing; `kelvins/municipios-brasileiros`;
`OHF-Voice/intents`. URLs are in the table above.
