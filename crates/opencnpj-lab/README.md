# opencnpj-lab

`opencnpj-intel` is the development CLI for building catalogues and aggregates,
generating datasets, training and evaluating interpreters, and benchmarking
query execution.

```sh
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo build -p opencnpj-lab
"$CARGO_TARGET_DIR/debug/opencnpj-intel" --help
```

The workspace skeleton lists planned subcommands. The
[roadmap](../../docs/intelligence/14-ROADMAP.md) defines when each is implemented.
