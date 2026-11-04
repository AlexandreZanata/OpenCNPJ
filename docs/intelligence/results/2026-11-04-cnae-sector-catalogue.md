# 2026-11-04 — WP-1.3 CNAE and sector catalogue [MEASURED]

Base commit `691330bb2c90f11b061c461a7beab630b718ecde`. Host Intel Core
i7-13620H, `performance` governor, no CPU pinning. Gate load averages
`3.29 7.42 8.43`. Compiler `rustc 1.99.0 (b940084d7 2026-09-28)`.
Offline builder `Python 3.12.3`. The
[CNAE manifest](../../../crates/opencnpj-insights/catalog/cnae_manifest.json)
identifies the exact IBGE, CONCLA and RFB input bytes and the output hashes.

## Commands

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
git rev-parse HEAD
lscpu | rg 'Model name'
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
cat /proc/loadavg
rustc -V
python3 -V
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
git diff --exit-code -- crates/opencnpj-core crates/opencnpj-store crates/opencnpj-ingest crates/opencnpj crates/opencnpj-desktop

raw=/data/opencnpj-data/intelligence/catalog-raw/2026-11-04/cnae
rfb=/data/opencnpj-data/intelligence/catalog-rfb/2026-10-verified
out=/data/opencnpj-data/intelligence/catalog-cnae/2026-10
python3 crates/opencnpj-lab/scripts/build-cnae.py --raw "$raw" --rfb "$rfb" --out "$out"
python3 crates/opencnpj-lab/scripts/build-cnae.py --raw "$raw" --rfb "$rfb" --out "$out-rebuild"
diff -r "$out" "$out-rebuild"
sha256sum "$out"/*
python3 /data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-verify.py
sha256sum /data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-top-50-review.md
```

## Raw output

- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-download.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-legacy-download.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-builder.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-verification.log`
- `/data/opencnpj-data/intelligence/results/2026-11-04-wp-1.3-gate.log`

## Summary

The delivery gate passed. The API and the official CONCLA structure agree
on 1,332 subclasses. The API provides 17,180 activity phrases. The catalogue
accounts for all 1,359 RFB reference codes, including 27 documented legacy
dispositions. RFB's `8888888` sentinel records unreported activity and is
kept as an explicit legacy row without a successor.

All CNAE sets in 210 curated sectors validated. The builder produced 33,783
automatic alias rows and reported 28 ambiguous phrases. It identified 24
codes without an unambiguous alias. The independent check verified that all
multiply assigned curated aliases appear in the report, including
`tecnologia` and `transportadora`. Both builds produced identical files.

The owner confirmed review and approval of the 50 priority sectors in the
execution chat on 2026-11-04. Priority is by the number of establishments
whose main CNAE is in a sector's core set, across statuses in the national
snapshot. A code can contribute to more than one sector. The reviewed sample
matches the generated `top-50.tsv` and has SHA-256
`4ef4564a042e49f9786aa32769e8302ae3df6e8acc7349f19a516929ad23bb5c`.

Correspondence rows describe the official scope changes and do not substitute
codes during execution. Early changes are documented in the exceptions TSV
with the CONCLA resolution, meeting minutes or explanatory notes. The
offline builder uses the Python standard library. No Rust dependencies were
added.
