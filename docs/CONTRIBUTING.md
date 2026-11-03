# Contributing

English code, comments and docs; tests for every change; conventional commits.

## Local setup

```bash
rustup toolchain install stable      # MSRV is declared in Cargo.toml
cargo test --workspace
```

No database or service is needed. Integration tests generate synthetic RFB-format ZIPs.
To work with real data:

```bash
cargo run --release -- download --dest data/raw
cargo run --release -- build --input data/raw/2026-10 --output data/2026-10.ocnpj
```

## Before opening a PR

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Changes to the snapshot layout must update [FORMAT.md](FORMAT.md) and bump the format
version (major for incompatible changes). Performance claims must come with the command
and the raw output (see [BENCHMARKS.md](BENCHMARKS.md)).

## Commits

[COMMIT_CONVENTION.md](COMMIT_CONVENTION.md): `type(scope): subject`, at most 72 characters.
