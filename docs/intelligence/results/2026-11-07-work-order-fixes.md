# 2026-11-07 — Work order results (PR #5 review fixes)

Work order `docs/intelligence/WORK-ORDER-2026-11-07.md`, branch
`docs/intelligence-plan`.

## Commits

```text
00bb8bf fix(lab): remove cloned items and wrong labels from frozen sets
1eb827e fix(lab): run the VPS emulation once and record its mode
3010b36 fix(lab): word-level abbreviations, single typos, spread coverage
06e894a fix(lab): evaluate dialogues and group equal confidences
d35f9f0 fix(query): drop exclusions that contradict or repeat the place
526ca50 fix(insights): verify aggregate tables and catalogue at open
d1f302d fix(lab): refuse lowering when a filter resolves to nothing
809c038 fix(insights): filter density denominators like the counts
3f58554 fix(insights): correct sector sets and add missing aliases
fe16f8a docs(intelligence): add the work order for the review fixes
```

(`git log --oneline fda63d7..HEAD`.)

## Per task

- Task 0: recorded the work order file.
- Task 1: corrected the transport core/extended split, added the missing sector aliases and codes, appended school, civil_construction (47 codes) and passenger_transport, updated the CNAE manifest.
- Task 2: density denominators now use the same place, population range and exclusions as the counts via PopulationScope.
- Task 3: engine lowering bails when a place or activity resolves to no code.
- Task 4: aggregate open checks for missing tables and a foreign municipality catalogue in diff, bench and historical artifacts.
- Task 5: canonicalization drops exclusions that repeat or contradict the place filter; regenerated the exclusions golden case.
- Task 6: the eval harness expands dialogues, groups equal confidences on the risk-coverage curve and returns the unnormalized AURC.
- Task 7: synthetic noise is word-level with single changes that never touch digits, templates use tem, coverage sampling is spread.
- Task 8: the VPS emulation script runs once and prints its mode to stderr.
- Task 9: frozen sets use core sector codes and 2026 estimates, dropped cloned items, fixed noise labels, deduplicated, clarified homonyms without UF, and moved WP-3.3 minimums to an ignored test.
- Task 10: this file.

## National differential

```bash
cargo build --release -p opencnpj-lab
OUT=/data/opencnpj-data/intelligence/results/$(date +%F)-work-order-diff.log
/data/opencnpj-target/release/opencnpj-intel diff-exec \
  --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --catalog crates/opencnpj-insights/catalog/municipalities.tsv \
  --sectors crates/opencnpj-insights/catalog/sectors.toml \
  --cases crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl > "$OUT" 2>&1; echo "exit=$?"
cat "$OUT"
```

Raw output (`exit=0`):

```text
match	padarias_sorriso
match	qtas_padarias_sorrisso_mt
match	compare_sorriso_com_sinop
match	sinop_vs_sorriso
match	farmacias_ativas_cuiaba
match	drogarias_em_funcionamento_cuiaba_mt
match	empresas_abertas_2025_mt
match	cnpjs_registrados_2025_mato_grosso
aggregate-only	estavam_ativas_2025_mt	Scalar { value: 557339 }
match	fabricacao_de_paes_sorriso
match	sorriso
match	sorriso_e_regiao
match	sem_filiais
match	so_filiais
match	compare_density_sorriso_sinop
aggregate-only	rank_sector_change_mt	Rows([Row { key: 192, value: 1930, base: 51703 }, Row { key: 194, value: 1284, base: 30042 }, Row { key: 113, value: 1141, base: 19906 }, Row { key: 132, value: 994, base: 11300 }, Row { key: 49, value: 828, base: 4514 }, Row { key: 109, value: 268, base: 4267 }, Row { key: 71, value: 249, base: 6221 }, Row { key: 120, value: 196, base: 3888 }, Row { key: 134, value: 166, base: 4747 }, Row { key: 180, value: 143, base: 4847 }])
match	compare_bakeries_pharmacies
match	union_bakeries_pharmacies
match	density_census_mt
engine-n/a	largest_capital_with_contact	engine executor does not serve this operation
engine-n/a	lookup_numeric	engine executor does not serve this operation
engine-n/a	lookup_alphanumeric_root	engine executor does not serve this operation
engine-n/a	lookup_alphanumeric_full	engine executor does not serve this operation
match	breakdown_porte
engine-only	monthly_closures	Rows([Row { key: 24315, value: 3438, base: 0 }, Row { key: 24316, value: 5839, base: 0 }, Row { key: 24317, value: 5366, base: 0 }, Row { key: 24318, value: 5337, base: 0 }, Row { key: 24319, value: 4980, base: 0 }, Row { key: 24320, value: 4945, base: 0 }, Row { key: 24321, value: 1515, base: 0 }])
engine-only	mei_opt_ins	Scalar { value: 84308 }
match	exclusions
match	rank_inhabitants_minimum
engine-only	annual_net_registrations	Rows([Row { key: 2025, value: 435832, base: 0 }, Row { key: 2026, value: 395497, base: 0 }])
static	methodology_meta
engine-only	closed_stock_filter	Scalar { value: 55505 }
aggregate-only	rank_change_percent	Rows([Row { key: 13, value: -27391, base: 543122 }, Row { key: 16, value: -5529, base: 244281 }, Row { key: 15, value: -16327, base: 761042 }, Row { key: 14, value: -2164, base: 353827 }, Row { key: 23, value: 10965, base: 1547619 }])
engine-only	cnae_main_or_secondary	Scalar { value: 44380 }
engine-only	head_offices_legal_nature	Scalar { value: 118415 }
match	near_sinop_km
match	rank_cnae_section
summary	compared=22	mismatches=0	aggregate_only=3	engine_only=6	engine_n/a=4	invalid=0
```

## Not done / stopped

- Nothing stopped: every task ran to its commit. See the final work order
  report for deviations and observations recorded along the way.
