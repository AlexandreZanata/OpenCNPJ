# 2026-11-04 — WP-0.1 workspace verification [MEASURED]

Base commit `7e0d00a5f780eda5fdfa4c653ac3d3b315126350` with the WP-0.1 changes applied.
Host Intel Core i7-13620H, `performance` governor, no CPU pinning. Load averages
`2.44 3.64 3.66` before the run. Compiler `rustc 1.99.0 (b940084d7 2026-09-28)`.
This run used the workspace test fixtures.

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
cargo build -p opencnpj-lab
"$CARGO_TARGET_DIR/debug/opencnpj-intel"
git diff --stat -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop
git diff --exit-code -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop
```

## Raw output

`/data/opencnpj-data/intelligence/results/2026-11-04-wp-0.1-workspace.log`

## Summary

The delivery gate and lab build passed. The executable printed the planned
subcommands. The diff checks for the existing crates produced empty output.
The lockfile adds workspace packages using dependencies already resolved by
the repository.
