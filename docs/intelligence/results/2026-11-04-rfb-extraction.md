# 2026-11-04 — WP-1.1 RFB reference extraction [MEASURED]

Base commit `01f9d834060fc40e68df9b14e841fc86ae704410` with the WP-1.1 changes
applied. Host Intel Core i7-13620H, `performance` governor, no CPU pinning.
Load averages `12.88 7.56 5.31` before the gate. Compiler
`rustc 1.99.0 (b940084d7 2026-09-28)`. Inputs were the existing
balanced test fixture and the 2026-10 national format 1.2 snapshot. The
national snapshot SHA-256 from its existing sidecar is
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`.

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
cargo test -p opencnpj-lab --test rfb_catalog fixture_rfb_catalog_matches_stats -- --exact --nocapture
git diff --check
git diff --exit-code -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop

export OPENCNPJ_RFB_SNAPSHOT=/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj
export OPENCNPJ_RFB_OUT=/data/opencnpj-data/intelligence/catalog-rfb/2026-10-verified
cat "$OPENCNPJ_RFB_SNAPSHOT.sha256"
cargo build --release -p opencnpj-lab
cargo test --release -p opencnpj-lab --test rfb_catalog --no-run
"$CARGO_TARGET_DIR/release/opencnpj-intel" catalog rfb --snapshot "$OPENCNPJ_RFB_SNAPSHOT" --out "$OPENCNPJ_RFB_OUT"
cargo test --release -p opencnpj-lab --test rfb_catalog national_rfb_catalog_matches_stats -- --ignored --exact --nocapture
sha256sum "$OPENCNPJ_RFB_OUT"/*.tsv
```

## Raw output

- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.1-gate.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.1-oracle-build.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.1-rfb-national.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.1-rfb-extraction.log` (first attempt)

## Summary

The delivery gate passed. The fixture and national runs matched every
exported code count and UF distribution against `/api/v1/stats`. The national
reference tables contained 5,572 municipality rows, 1,359 CNAE rows and 92
legal-nature rows. The national index had 5,572 used municipality terms,
1,343 main CNAEs and 1,350 secondary CNAEs. The 5,572 indexed municipality
terms are the observed inputs for WP-1.2. The prior audit reports 5,583 seen
codes from its earlier analysis.

## Notes

The initial fixture oracle filtered by all indexed statuses. The engine also
stores documents for company roots without establishments, which have no
status. The test now selects every CNPJ prefix and checks the filter against
the snapshot document count through `/api/v1/search` before comparing every
code and UF distribution with `/api/v1/stats`.

Secondary CNAE totals sum document-code pairs. One establishment can have
multiple secondary codes. The code exports and tests reuse dependencies
already resolved by the workspace.
