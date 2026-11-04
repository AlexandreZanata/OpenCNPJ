# 2026-11-04 — WP-1.2 municipality catalogue [MEASURED]

Base commit `2a887b5` (WP-1.1). Host Intel Core i7-13620H, `performance`
governor, no CPU pinning. Gate load averages `11.12 6.71 6.32`. Compiler
`rustc 1.99.0 (b940084d7 2026-09-28)`. The check uses the verified RFB extraction of
the national 2026-10 snapshot and the official sources retrieved on
2026-11-04. Their URLs, byte sizes, SHA-256 values and terms notes are in
`/data/opencnpj-data/intelligence/catalog-raw/2026-11-04/manifest.json`.

## Initial source coverage command

```bash
python3 /data/opencnpj-data/intelligence/results/2026-11-04-wp-1.2-preflight.py \
  /data/opencnpj-data/intelligence/catalog-raw/2026-11-04 \
  /data/opencnpj-data/intelligence/catalog-rfb/2026-10-verified
```

## Raw output

The command exited with status 1. Full output, including source hashes, is
in `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.2-preflight.log`.
The download commands and responses are in the `2026-11-04-wp-1.2-*.log`
files in that directory. The relevant output is reproduced below.

```text
used TOM codes: 5572
used TOM codes absent from Tesouro: ['1182', '9707']
used TOM codes missing after roadmap overrides: []
source municipalities with a numeric 2022 census population: 5570
source municipalities with a numeric 2026 population estimate: 5571
municipality with zero recorded neighbours: 2605459 Fernando de Noronha
municipality with zero recorded neighbours: 3520400 Ilhabela
raw neighbour pairs: 32994
raw neighbour pairs without reverse: 0
spot check: 7107 3550308 São Paulo SP
spot check: 9907 5107925 Sorriso MT
spot check: 8985 5107909 Sinop MT
missing required attributes: 1182 5101837 Boa Esperança do Norte MT documents: 622 estimate: 5983 missing: 2022 census population
WP-1.2 acceptance failed: every mapped municipality must have the required attributes
```

## Source decision

The planned overrides cover the two codes missing from Tesouro. `9707`
represents establishments abroad. `1182` maps to Boa Esperança do Norte,
which has a name, UF, seat coordinates and a 2026 population estimate. Its
municipal census population is absent from SIDRA 4709/93 for 2022. IBGE lists
the municipality as officially installed on 2025-01-01 and identifies the
earlier population figure on its municipality page as an estimate.
[IBGE municipality page](https://www.ibge.gov.br/cidades-e-estados/mt/boa-esperanca-do-norte.html)
[SIDRA census source](https://servicodados.ibge.gov.br/api/v3/agregados/4709/periodos/2022/variaveis/93?localidades=N6%5Ball%5D)

The owner approved using the official IBGE estimate for this municipality.
Its census field is unavailable in the catalogue. Its estimate is 5,983,
with reference year 2026 and source SIDRA 6579/9324. The loader requires a
census value for all other municipalities. The initial preflight failure above
records the original acceptance criterion before this exception was approved.
[SIDRA estimate source](https://servicodados.ibge.gov.br/api/v3/agregados/6579/periodos/2026/variaveis/9324?localidades=N6%5Ball%5D)

The boundary workbook also includes the state operational areas for Lagoa
Mirim (`4300001`) and Lagoa dos Patos (`4300002`). The first catalogue build
stopped on an unknown neighbour from one of these areas. The municipality
graph now projects the official source onto municipal and equivalent district
codes, with the two operational codes recorded in the manifest. Other
unclassified codes fail the build. An independent comparison checked the
entire projected edge set against the workbook.
[IBGE Municipal Mesh](https://www.ibge.gov.br/geociencias/organizacao-do-territorio/malhas-territoriais/15774-malhas.html?edicao=42093&t=acesso-ao-produto)

## Acceptance commands

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
git rev-parse HEAD
lscpu | rg 'Model name'
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
cat /proc/loadavg
rustc -V
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
git diff --exit-code -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop

export OPENCNPJ_MUNICIPAL_SNAPSHOT=/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj
export OPENCNPJ_MUNICIPAL_CATALOG=/data/opencnpj-data/intelligence/catalog-municipalities/2026-10
export OPENCNPJ_MUNICIPAL_REBUILD=/data/opencnpj-data/intelligence/catalog-municipalities/2026-10-rebuild
raw=/data/opencnpj-data/intelligence/catalog-raw/2026-11-04
cargo build --release -p opencnpj-lab
"$CARGO_TARGET_DIR/release/opencnpj-intel" catalog municipalities --snapshot "$OPENCNPJ_MUNICIPAL_SNAPSHOT" --raw "$raw" --out "$OPENCNPJ_MUNICIPAL_CATALOG"
"$CARGO_TARGET_DIR/release/opencnpj-intel" catalog municipalities --snapshot "$OPENCNPJ_MUNICIPAL_SNAPSHOT" --raw "$raw" --out "$OPENCNPJ_MUNICIPAL_REBUILD"
cargo test --release -p opencnpj-lab --test municipality_catalog national_municipality_catalog_is_complete_and_reproducible -- --ignored --exact --nocapture
sha256sum "$OPENCNPJ_MUNICIPAL_CATALOG"/*
python3 /data/opencnpj-data/intelligence/results/2026-11-04-wp-1.2-verify-sources.py "$raw" "$OPENCNPJ_MUNICIPAL_CATALOG"
```

## Acceptance raw output

- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.2-gate.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.2-national.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.2-source-check.log`

## Summary

The delivery gate passed. The catalogue covers all 5,572 TOM codes used in
the snapshot and passed the São Paulo, Sorriso and Sinop spot checks. The
5,571 geographic rows match the official IBGE names, UFs, regions and
population fields. The graph contains all 32,952 symmetric directed links
between municipalities and equivalent districts. The 42 links involving
operational areas remain in the raw workbook. Two builds from the same
inputs produced byte-identical output and manifest files.

The full catalogue and downloads remain under `/data`. The repository
includes the [source and artifact manifest](../../../crates/opencnpj-insights/catalog/manifest.json)
and [22 RFB/IBGE name differences](2026-11-04-rfb-ibge-name-differences.tsv)
for review.

The builder uses versions already resolved by the workspace. `quick-xml`
0.41.0 (MIT) reads the ODS XML, `zip` 9.3.0 (MIT) opens the ODS and GeoPackage
archives, and `rusqlite` 0.40.2 (MIT) queries the GeoPackage. The existing
snapshot and ingest APIs supply the RFB codes, names and SHA-256 helper.
