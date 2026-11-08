# Final implementation report - official data, MT first

Date: 2026-11-08. Branch: `codex/official-data-mt`, based on
`origin/codex/official-data-mt-plan` (`27c2b92`). The branch was pushed to
`origin` after each milestone.

## Integrated and queryable

| Source | Dataset | What is queryable |
|---|---|---|
| IBGE | `ibge-6579` | annual population estimates, 2001-2026 |
| IBGE | `ibge-9922` | 2022 Census households, residents and average household size |
| IBGE | `ibge-5938` | municipal GDP and value added, 2002-2023 |
| IBGE | `ibge-9509` | CEMPRE local units, organizations, employment, payroll and average wage, 2022-2024 |
| IBGE | `ibge-1612` | temporary-crop area, production, yield and value, six products, 2020-2025 |
| IBGE | `ibge-1613` | coffee area, production, yield and value, 2020-2025 |
| IBGE | `ibge-3939` | bovine, swine, poultry and sheep herds, 2020-2025 |
| IBGE | `ibge-6588` | monthly LSPA soybean area/production for Mato Grosso state |
| MTE | `mte-rais-2024` | annual formal jobs by MT municipality and six activity groups, 2023-2024 |
| Senatran | `senatran-frota-2026-09` | registered fleet by MT municipality and 22 vehicle types, 202609 |

Every slice has: immutable `.ocind` store, embedded manifest, raw evidence hash,
source/licence/attribution metadata, typed operations (`lookup`, `compare`,
`rank`, `trend`, `ratio`), Portuguese interpretation and source-aware answers.

## Commercially enabled

None. IBGE, MTE and Senatran remain `review_required`; ANEEL is ODbL and ANTT's
checked resource declares CC Attribution. The registry records the actual state;
no blanket open licence or commercial clearance is claimed.

## Not integrated / external blockers

- Novo Caged monthly aggregates: no admitted stable small municipal endpoint;
  PDET FTP microdata and Google Drive tables were not used to fabricate a series.
- ANTT RNTRC: aggregate and identified resources have different schemas; no
  CNPJ/fleet ownership link.
- PNCP, Compras.gov.br, CGU CEIS/CNEP: endpoints rechecked but contract-level
  stores, deduplication, amount semantics and sanction scope are not implemented.
- CNES, INEP, ANP, Comex Stat, BCB, ANEEL and PGFN: revalidated, not admitted.
- MT state portal: no individually verified dataset admitted.
- Cross-source CNPJ-establishment ratios and correlation coefficients: require
  an explicit operation; not implemented.

## Data evidence

Raw responses and `.ocind` stores are outside Git under
`/data/opencnpj-data/intelligence/official/`. The source registry, per-asset
hashes and selected official cell values are in `SOURCES.md` and `REAL-DATA.md`.
No national dataset or large binary was committed.

## Additional source and CNPJ crosswalk audit

A second pass revalidated all 15 registered sources with 24 bounded probes and
categorized the received bytes (JSON, XLSX, CSV, HTML, truncated sample) without
normalizing them in place. The CNPJ T1/T2 municipality marginals were compared
with the versioned IBGE catalogue and every admitted `.ocind` geography.

- 0 CNPJ municipality IDs outside the catalogue.
- 0 missing-municipality sentinel cells.
- 0 duplicate folded names in the same UF.
- 0 official N6 geography codes without a catalogue match.
- Remaining items are source coverage/naming/availability tasks, not CNPJ
  identity mismatches: Boa Esperança do Norte census/CEMPRE/PIB gaps, LSPA
  state-only scope, 16 generated Senatran vehicle IDs, PGFN malformed links and
  variable ANP availability.

Evidence: `SOURCE-AUDIT.md`, `INCONSISTENCIES.md`,
`audit/source-audit.json`, `audit/cnpj-crosswalk-audit.json`.

## Validation

| Gate | Result |
|---|---|
| `cargo fmt --all --check` | exit 0 |
| `cargo clippy --workspace --all-targets -- -D warnings` | exit 0 |
| `cargo test --workspace` | 323 passed, 0 failed (29 suites, including doc tests) |
| MT acceptance corpus | 261/261 passed |
| HTTP API smoke | `/api/v1/official/sources` and `/api/v1/official/query` returned source-backed answer |

Logs: `logs/stage-0-*` through `logs/stage-10-*`.

## HTTP surface

`opencnpj serve --official-dir DIR` exposes:

- `GET /api/v1/official/sources` (public registry/status);
- `GET /api/v1/official/query` (gated typed indicator query).

`docs/API.md` documents parameters and responses. No arbitrary SQL endpoint was
introduced, and no claim is made that the local CLI state file is signed HTTP
context.

## Commits pushed

```
d973f05 docs(docs): record official data integration baseline
f5aa477 feat(ingest): add verified source asset lifecycle
857375b feat(ingest): import and query MT IBGE indicators
53a417b docs(docs): record MT IBGE real-data checks
c467a92 feat(ingest): add RAIS and Senatran XLSX imports
a771d68 docs(docs): record MT labour agriculture and fleet results
19b4058 test(cli): cover MT official-data acceptance boundaries
```

Stage 10 adds the HTTP surface and final report in its own commit/push.

## Remaining for human refinement

- Product acceptance of the Portuguese answer wording and operational UX.
- Legal review of IBGE/MTE/Senatran reuse terms; ANEEL ODbL architecture.
- Decide whether to implement CAGED, procurement, health, education, fuel,
  trade, energy and fiscal verticals, each with its own admitted source store.
- Human review of the 261-case corpus and addition of real user phrasings.
- Do not open or tune against H-blind.
