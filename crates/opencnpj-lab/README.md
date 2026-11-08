# opencnpj-lab

`opencnpj-intel` is the development CLI for building catalogues and aggregates,
generating datasets, training and evaluating interpreters, and benchmarking
query execution.

```sh
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo build -p opencnpj-lab
"$CARGO_TARGET_DIR/debug/opencnpj-intel" --help
```

The [roadmap](../../docs/intelligence/14-ROADMAP.md) defines when each
subcommand is implemented.

## Extract RFB reference tables

```sh
cargo build --release -p opencnpj-lab
"$CARGO_TARGET_DIR/release/opencnpj-intel" catalog rfb \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --out /data/opencnpj-data/intelligence/catalog-rfb/2026-10
```

The command publishes the export as a directory. Use an absent or empty
destination. The three `rfb_*.tsv` files contain the snapshot's reference rows
as `code` and `label`. Fields with tabs, newlines or quotes use standard CSV
quoting with a tab separator.

`used_codes.tsv` contains the indexed municipality, main CNAE and secondary
CNAE codes, including codes absent from the reference tables. Each row gives
the count of distinct documents for its field and code. `uf_distribution` is
a JSON object keyed by the indexed UF. The empty key counts documents with
no indexed UF. A document can contribute to several secondary CNAE rows.

## Build the municipality catalogue

```sh
cargo build --release -p opencnpj-lab
"$CARGO_TARGET_DIR/release/opencnpj-intel" catalog municipalities \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --raw /data/opencnpj-data/intelligence/catalog-raw/2026-11-04 \
  --out /data/opencnpj-data/intelligence/catalog-municipalities/2026-10
```

The raw directory contains `manifest.json` with URLs, sizes, SHA-256 hashes
and usage notes for the Tesouro and IBGE files. The command checks the raw
files against that manifest, reads all used TOM codes from the snapshot, and
publishes a verified catalogue to an absent or empty destination. The output
includes `municipalities.tsv`, `overrides.tsv`, `rfb-name-differences.tsv`, and
a deterministic `manifest.json` identifying the snapshot, sources and outputs.

## Build the CNAE and sector catalogue

Use the verified WP-1.1 extraction for the RFB input. No Python is needed (the former
`build-cnae.py` was ported; its outputs were byte-identical).

```sh
opencnpj-intel catalog cnae \
  --raw /data/opencnpj-data/intelligence/catalog-raw/2026-11-04/cnae \
  --rfb /data/opencnpj-data/intelligence/catalog-rfb/2026-10-verified \
  --out /data/opencnpj-data/intelligence/catalog-cnae/<new-directory>
```

The output directory contains the hierarchy, sector definitions, automatic
alias candidates, ambiguity and reachability reports, and a source manifest.
Copy `cnae.tsv`, `ambiguity.tsv`, `reachability.tsv`, `top-50.tsv` and
`manifest.json` (as `cnae_manifest.json`) into `opencnpj-insights/catalog`;
`sector_aliases_auto.tsv` (2.4 MB) stays under `/data`.

## Historical backfill (WP-1.5)

```sh
opencnpj-intel agg backfill --first 2024-11 --last 2026-10     # one month at a time, Linux only
opencnpj-intel agg backfill --summarize                       # validate archives, national totals
```

## Build and verify aggregates

```sh
cargo build --release -p opencnpj-lab
D=/data/opencnpj-data/intelligence
S=/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj
C=$D/catalog-municipalities/2026-10/municipalities.tsv
B=$CARGO_TARGET_DIR/release/opencnpj-intel
"$B" agg build --snapshot "$S" --catalog "$C" --out "$D/agg/2026-10.ocagg"
"$B" diff-agg --snapshot "$S" --catalog "$C" --agg "$D/agg/2026-10.ocagg" --sample 10000 --seed 42
```

The builder expects a verified snapshot. Select `--tables T1,T2` for a
historical stock artefact. The comparison checks complete CNAE, UF and
municipality marginals and queries exact engine counts for sampled cells.
See the [aggregate format](../../docs/intelligence/AGGREGATES-FORMAT.md).

## Portuguese questions and commercial evaluation

`opencnpj-intel ask --text 'Quantas padarias ativas existem em Sorriso/MT?'`
interprets a question. Add `--agg`, `--history`, `--snapshot` and optional
`--state` to execute and continue a local conversation.

`opencnpj-intel eval-questions --set <questions.jsonl> --out <directory>`
records actual text-only interpretations, canonical label comparisons and,
when real data paths are provided, separate predicted/gold execution audits.
`--interpret-only` skips backend execution. The runner never seeds predictions
or conversation state with labels. See the
[commercial evaluation runbook](../../docs/intelligence/results/commercial-round-01/RUNBOOK.md).
