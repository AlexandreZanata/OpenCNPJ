# 2026-11-06 — WP-3.1 synthetic generator [MEASURED]

Base commit `1139f59fec57bbd788c20d3905168d6f085eed2b`. Compiler
`rustc 1.99.0 (b940084d7 2026-09-28)`.

## Delivery

`opencnpj-lab::synth` builds pt-BR synthetic evaluation items from the
versioned family specs and the real municipality and sector catalogues:

* `crates/opencnpj-lab/synth/pt-BR/families.toml`: **62 families**, **496
  templates** (8 per family, two synthetic authors per family).
* Frame sampler with real catalogue slot fillers, Portuguese realization
  (gender, contractions, articles), gold IR built from the frame and
  canonicalized with the production validator.
* Noise model: register, abbreviations, accent removal, typos, case,
  punctuation, fillers and fragments; phonetic swaps and merged words are
  reserved for the test split. Every item records the applied transforms.
* Splits by family (70/15/15), by slot value (20 % test-only entities) and
  by reserved noise; deduplication on the delexicalized form with at most
  three examples per form and split; cross-split forms removed from train.
* Dialogue generator (`dialogues.jsonl`) and the manifest with SHA-256 of
  the spec and catalogue, counts per family/intent/noise/split and the
  diversity metrics from 10-SYNTHETIC-DATA.md §9.
* CLI `opencnpj-intel synth`, which also writes the 200-item human review
  sample.

## Generated dataset

```text
dataset synth-0.1.0-f25aa83b-42
2381 items + 400 dialogues
train 1613, val 378, test 390
62 families, 496 templates
token entropy 8.294, char 4-gram entropy 11.578
slot coverage: municipalities 31.2 %, sectors 100 %
```

The dataset lives under
`/data/opencnpj-data/intelligence/datasets/synth-0.1.0-f25aa83b-42/`
(`train.jsonl`, `val.jsonl`, `test.jsonl`, `dialogues.jsonl`,
`manifest.json`, `review-sample.jsonl`, `review-sample.md`). It is a build
artifact under `/data`, not committed.

Selected items:

```text
quero ver os bares c/ entretenimento de Santo Antônio do Sudoste
compare bar e gráfica publicitária em Antônio Carlos (MG)
quantos serviços de revestimento de metais há no território nacional por favor
conta as empresas de coleta de resíduos comuns que abriram em Rio das Flores nos últimos noventa dias
separa as lojas de cosméticos por situação em Turmalina (SP)
```

## Acceptance

* **Determinism.** `synth::tests::determinism` builds the fixture twice
  and compares the five files. On the national catalogue the CLI was run
  twice (dataset and `datasets-check`); `diff -rq` reports byte-identical
  files. Log:
  `/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.1-determinism.log`.
* **Split leakage.** `synth::tests::splits_do_not_leak` checks that
  test-only municipalities and sectors, test families, reserved noise
  types and delexicalized forms do not cross into train/validation.
* **Gold IR.** Every generated executable gold is canonicalized and
  validated with `opencnpj_query::validate`; `synth::tests::every_gold_ir_validates`
  re-validates every item and dialogue turn.
* **Manifest diversity.** `synth::tests::manifest_reports_diversity`
  checks the distinct-form, template, slot-coverage, token/character
  entropy, Jaccard and noise-share fields; the national manifest carries
  the values above.
* **Human naturalness sample.** The 200-item random sample is written to
  `review-sample.md` with a natural/unnatural checkbox per item. The
  human review and the < 5 % target are **pending**; the share is not yet
  recorded. The sample is ready for the owner's review.

## Commands

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
cargo build --release -p opencnpj-lab
"$CARGO_TARGET_DIR/release/opencnpj-intel" synth \
  --per-family 50 --seed 42 --level l3 --review 200
"$CARGO_TARGET_DIR/release/opencnpj-intel" synth \
  --per-family 50 --seed 42 --level l3 --review 200 \
  --out /data/opencnpj-data/intelligence/datasets-check
diff -rq \
  /data/opencnpj-data/intelligence/datasets/synth-0.1.0-f25aa83b-42 \
  /data/opencnpj-data/intelligence/datasets-check/synth-0.1.0-f25aa83b-42
cargo test -p opencnpj-lab --bin opencnpj-intel synth::tests
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

Raw logs:

* `/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.1-synth.log`
* `/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.1-determinism.log`
* `/data/opencnpj-data/intelligence/results/2026-11-06-wp-3.1-gate.log`

The delivery gate passed (fmt, clippy with `-D warnings`, workspace tests;
the five synth tests are green).

## Limits and follow-ups

* The human review of the 200-item sample is the remaining acceptance
  step; the templates are AI-assisted and carry two synthetic author
  labels, but the review has not been recorded here.
* The manifest records the git commit at build time. The dataset above
  was built from the working tree at the base commit; the generator commit
  follows. A rerun from the same generator version and seed is
  byte-identical (verified above); a code change requires a generator
  version bump.
* The family templates are deliberately repetitive at this stage; the
  review sample is the feedback loop for a naturalness pass before the
  learning-curve experiments.
