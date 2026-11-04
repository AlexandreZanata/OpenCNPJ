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

The offline builder uses Python 3.11 or later and the standard library.
Use the verified WP-1.1 extraction for the RFB input.

```sh
python3 crates/opencnpj-lab/scripts/build-cnae.py \
  --raw /data/opencnpj-data/intelligence/catalog-raw/2026-11-04/cnae \
  --rfb /data/opencnpj-data/intelligence/catalog-rfb/2026-10-verified \
  --out /data/opencnpj-data/intelligence/catalog-cnae/2026-10
```

The output directory contains the hierarchy, sector definitions, automatic
alias candidates, ambiguity and reachability reports, and a source manifest.
The generated artefacts remain under `/data`. The curated sector definitions,
legacy exceptions and catalogue manifest live in `opencnpj-insights/catalog`.
