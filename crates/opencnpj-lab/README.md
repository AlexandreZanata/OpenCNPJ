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
