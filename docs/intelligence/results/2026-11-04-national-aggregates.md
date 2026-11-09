# 2026-11-04 — WP-1.4 national aggregates [MEASURED]

Base commit `4364c8538ccf21d0fd5a14a4736c8dc5903d5a71`. Host Intel Core
i7-13620H, `performance` governor, no CPU pinning. Gate load averages
`17.90 14.72 11.25`. Compiler `rustc 1.99.0 (b940084d7 2026-09-28)`.
Inputs were the existing balanced fixture and national snapshot 2026-10
with SHA-256 `76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`.
The municipality catalogue SHA-256 is
`d0bc5bbe308d35ae3b7913e617f0928ca5ac187464c49d517e11b4048d74c2b5`.

## Commands

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
cargo test -p opencnpj-lab --test aggregate_fixture -- --nocapture
git diff --check
git diff --exit-code -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop

snapshot=/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj
catalogue=/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv
out=/data/opencnpj-data/intelligence/agg/2026-10.ocagg
cargo build --release -p opencnpj-lab
/usr/bin/time -v "$CARGO_TARGET_DIR/release/opencnpj-intel" agg build --snapshot "$snapshot" --catalog "$catalogue" --out "$out"
stat -c '%s %n' "$out"
sha256sum "$out"
/usr/bin/time -v "$CARGO_TARGET_DIR/release/opencnpj-intel" diff-agg --agg "$out" --snapshot "$snapshot" --catalog "$catalogue" --sample 10000 --seed 42
```

## Raw output

- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.4-gate-final.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.4-national.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.4-build.log` (preliminary encoding)

## Summary

The delivery gate passed. The fixture matched the decoded source rows for
every cell and marginal, and its CNAE offsets retrieved the same rows as a
full scan. Corrupted, truncated and invalid-directory artefacts were rejected.

On the national snapshot, all CNAE, UF and municipality marginals for T1–T4
matched the engine. The seeded differential run checked 10,000 cells across
T1–T5 with zero mismatches. T2 accounted for all 73,809,379 establishments.
The manifest recorded four missing registration dates and no missing closure
dates. T5 reports document-code pairs, so its total is multivalued.

The final file is 51,341,928 bytes, within the 30–60 MB **[ESTIMATE]** from
13-RESOURCE-ESTIMATES.md. Building it took 20.15 seconds wall time, including
hashing the input and output, with peak RSS of 4,688,108 KiB. The earlier
design target of under five minutes was a **[HYPOTHESIS]**. The current
measurement is a build-host run with page cache influenced by the preceding
build. The RSS is for the builder process and is distinct from the chat node
estimates.

The implementation reuses the workspace's memmap2, xxHash, Zstandard, rayon,
roaring and serde packages. The builder retains the UF indexed on the
establishment and encodes missing dimensions explicitly. The aggregate
file and its SHA-256 sidecar are stored under `/data`.
