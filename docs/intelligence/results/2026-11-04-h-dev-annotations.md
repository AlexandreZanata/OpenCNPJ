# 2026-11-04 — WP-0.2 H-dev verification [MEASURED]

Base commit `4f18400e3ecda118ac24de0185eefd18157e7059` with the WP-0.2 changes
applied. Host Intel Core i7-13620H, `performance` governor, no CPU pinning.
Load averages `2.78 3.68 3.81` before the run. Compiler `rustc 1.99.0 (b940084d7 2026-09-28)`.

H-dev SHA-256 `c5a98077a6470e3b15a9c4c988be6be34e9965dbc8f2ce057cfc27bf45e45ebd`.
Review sample SHA-256 `169dc00a8cd6c3d86be4304677ddfd1810e070ce0b9c2caafbe3fcb4c14ebc65`.

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
cargo test -p opencnpj-lab --test h_dev h_dev_matches_annotation_ir -- --ignored --exact --nocapture
rg -c '^[0-9]+\. \*\*“' docs/intelligence/ANNOTATION-GUIDE.md
rg -c '^## [0-9]+\. h-dev-' /data/opencnpj-data/intelligence/eval/h-dev-review-50.md
rg -c '"origin":"human-reviewed"' /data/opencnpj-data/intelligence/eval/h-dev.jsonl
sha256sum /data/opencnpj-data/intelligence/eval/h-dev.jsonl /data/opencnpj-data/intelligence/eval/h-dev-review-50.md
git diff --check
git diff --exit-code -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop
```

## Raw output

`/data/opencnpj-data/intelligence/results/2026-11-04-wp-0.2-h-dev-reviewed.log`

## Summary

The guide contains 45 worked examples. The annotation IR test passed for all
317 H-dev items. The executable decisions cover each intent, with counts from
34 to 49. The test checks all dialogue acts, metrics, flows and semantic rule
tags. These are corpus counts rather than interpreter quality measurements.

On 2026-11-04 the owner confirmed review and approval of all 50 sampled labels
in the execution chat. The sample covered all intents and included clarification,
rejection and dialogue cases. The 50 corresponding items are marked
`human-reviewed`. The remaining 267 items are `ai-draft`. The reviewed IDs are
listed in `/data/opencnpj-data/intelligence/eval/h-dev-reviewed-ids.txt`.

The corpus, sample and authoring inputs live under
`/data/opencnpj-data/intelligence/eval/`. The validation test uses a review
schema derived from the query IR design while the production types are pending
WP-2.1. It reuses the workspace's `serde` and `serde_json`, plus the existing
core CNPJ and store calendar routines.
