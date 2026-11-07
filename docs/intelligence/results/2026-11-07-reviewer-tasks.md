# 2026-11-07 — Review of the work order and the four reviewer tasks [MEASURED]

Branch `docs/intelligence-plan` (PR #5), commits `6902026` … `0a2c480` on top of the work
order commits `fe16f8a` … `121b8e8`. Host: i7-13620H, `CARGO_TARGET_DIR=/data/opencnpj-target`,
national snapshot `2026-10-balanced-1.2.ocnpj`. Raw logs are under
`/data/opencnpj-data/intelligence/results/` (named below).

## 1. Review of the work order (11 commits by another model)

Every task matched `WORK-ORDER-2026-11-07.md`; the delivery gate and the national
differential passed as reported. One defect: to make a test pass, Task 6 changed the
baseline's expected AURC to 5/3 — an impossible value (coverage above 1). The root cause
was the earlier review's change that put executed clarify/reject items on the
risk–coverage curve without adding them to its denominator. Fixed in `6902026`; the
baseline now has one point (coverage 1, risk 1) and AURC 1.

Noted, not defects: the three "unknown" sector codes in Task 1's validation are legacy RFB
codes kept deliberately (`bakery` 4721101, `beauty_care` 9609201, `dental_clinic` 8630505);
the new catalogue alias "construtora" collided with `construction`'s display label (fixed
in §4).

## 2. Task: independent engine differential (`f7c8338`)

`diff-exec` shared the executor's place resolution, population sums and CNAE section
table, so a bug there passed on both sides. New `crates/opencnpj-lab/src/oracle.rs`
re-derives places (IBGE macro-regions written out, neighbours, great-circle distance) and
populations from the catalogue, and sections are counted with the engine's own
`cnae=<letter>` filter. Past competences and active-change rankings are checked against
the artifacts' raw marginal sections instead of being skipped. `diff-agg` now also checks
the UF × CNAE marginals.

```bash
B=/data/opencnpj-target/release/opencnpj-intel
$B diff-exec --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --history /data/opencnpj-data/intelligence/backfill/aggregates \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --catalog crates/opencnpj-insights/catalog/municipalities.tsv \
  --sectors crates/opencnpj-insights/catalog/sectors.toml \
  --cases crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl
/usr/bin/time -v $B diff-agg --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
  --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
  --catalog crates/opencnpj-insights/catalog/municipalities.tsv --sample 10000 --seed 42
```

Raw output (`2026-11-07-python-free-diff-exec.log`, `2026-11-07-independent-diff-agg.log`):

```text
marginal-match	estavam_ativas_2025_mt
marginal-match	rank_sector_change_mt
marginal-match	rank_change_percent
summary	compared=28	mismatches=0	aggregate_only=0	engine_only=6	engine_n/a=4	invalid=0

UfCnae(T1): 31423 groups match the engine
UfCnae(T2): 33867 groups match the engine
UfCnae(T3): 33867 groups match the engine
UfCnae(T4): 31081 groups match the engine
seed 42, 10000 sampled cells, 0 mismatches
	Elapsed (wall clock) time (h:mm:ss or m:ss): 0:27.36
```

Before: `compared=22`, three cases `aggregate-only`. Three golden cases were added
(`breakdown_cnae_section_brazil`, `count_section_i_mt`, `density_small_towns_mt`) because
the existing set never touched section I or a population-filtered density.

**Mutation runs** (bug re-introduced in `execute.rs`, oracle run, code restored):

```text
# section 56 moved back to J
MISMATCH	breakdown_cnae_section_brazil	aggregate=Rows([Row { key: 65, value: 931460, ... }, ...])
MISMATCH	count_section_i_mt	aggregate=Scalar { value: 2453 }	engine=Scalar { value: 39752 }
# density denominator without the population range
MISMATCH	density_small_towns_mt	aggregate=Ratio { num: 94, den: 3950330 }	engine=Ratio { num: 94, den: 1526685 }
```

## 3. Task: no Python needed to rebuild (`45cf725`)

| Former script | Replacement | Equivalence check |
|---|---|---|
| `scripts/build-cnae.py` | `opencnpj-intel catalog cnae` | Same inputs → `diff -r` of the two output directories: **identical**, all 7 files including `manifest.json` |
| `scripts/backfill-aggregates.py` | `opencnpj-intel agg backfill` | `--summarize` over the 24 archives: **identical** output (also re-verifies each archive's SHA-256); the build path is a line-by-line port (same commands, checks and manifest field order), not re-run end to end |
| `eval/build-sets.py` (+ generated `context.json`) | none: the JSONL files are the source of truth | The set test validates every gold against the committed catalogues; card rows refresh with `UPDATE_CARD=1` |

New dependency: `unicode-normalization` 0.1 (MIT/Apache-2.0), needed to reproduce Python's
NFKD folding exactly. Found and fixed on the way: the lab's `load_sectors` also read
`cnae_extended`, so lab sectors (both sides of `diff-exec`) were wider than the catalogue's
core definition.

## 4. Task: reviewer pass on the "owner approvals" (`de8b835`)

**This is a review by the model, not the owner's approval.** Owner confirmation remains
pending for the H-dev sample, the priority sectors and the Boa Esperança do Norte estimate.

* **H-dev sample (50 labels, independent reviewer):** 47 OK, 0 wrong, 3 doubtful
  (h-dev-0117, h-dev-0118, h-dev-0183: IR correct, only "execute" vs "execute with note"
  or the note's wording is debatable).
* **Priority sectors (top 50, read code by code):** core sets correct (e.g. bakery's
  1091-1/02 is the CONCLA successor of retail-production bakeries). Fixed ambiguities:
  "construtora" (now `construction` only), "transportadora" (now `truck_freight` only;
  annotation-guide example 29 and two H-dev items follow), "estética" (now `beauty_care`;
  9602-5/02 moved to the salon's extension), "mercado" (now deliberately ambiguous between
  grocery and supermarket, so the assistant asks).
* **ADV golds:** 68 registration golds ("abriram em 2025") counted only *active*
  establishments, contradicting SR-02; they now count every status and the set test
  enforces it.

## 5. Task: new evaluation questions (`0a2c480`)

Written by hand by the model (`meta.origin = "claude-review-draft"`), not by the generator:
69 ADV items, 122 informal items (real typos, missing accents, abbreviations, informal
agreement, regional forms, each labelled with the noise it actually contains), 12 dialogues
(including `remove_filter` and `answer_clarification`). Three golds that ranked
municipalities with an invented capital filter became capital-ordered lists.
**H-blind was not touched** — it must come from people (10-SYNTHETIC-DATA.md §6).

```text
adv 304 · h-informal 202 · dialogues 60 (122 turns) · ood 400
cargo test -p opencnpj-lab --bin opencnpj-intel wp_3_3 -- --include-ignored   → ok
```

Baseline evaluation (always "count", no entities), all four sets load and score:

```text
adv.jsonl:        items 304 in-domain 243 executed 243 exact 0.000 risk 1.000 coverage 1.000 aurc 1.000 ece 0.500
ood.jsonl:        items 400 in-domain 0   executed 0   exact 0.000 risk 1.000 coverage 0.000 aurc 1.000 ece 0.500
h-informal.jsonl: items 202 in-domain 165 executed 165 exact 0.000 risk 1.000 coverage 1.000 aurc 1.000 ece 0.500
dialogues.jsonl:  items 122 in-domain 104 executed 104 exact 0.000 risk 1.000 coverage 1.000 aurc 1.000 ece 0.500
```

## 6. Delivery gate (last run before `0a2c480`)

`cargo fmt --all --check` ok · `cargo clippy --workspace --all-targets --locked -- -D warnings`
ok · `cargo test --workspace --locked`: 258 passed, 0 failed, 5 ignored.

## 7. Still open after this round

1. **Human-written evaluation data.** H-blind (WP-0.3) does not exist; ADV/informal/dialogues
   are AI-drafted, so WP-3.3 stays formally open and WP-5.1 cannot use them.
2. **Owner confirmations** listed in §4.
3. **Phase 4 not started:** `opencnpj-nlu` is still empty; this is the product's core.
4. From the 2026-11-07 review, still open: `validate()` accepts non-canonical requests
   (suggest a `CanonicalRequest` input type); evaluation exact match compares raw JSON
   instead of canonical IR; entity P/R has no spans; renderer template lint uses one global
   schema; `Sources` cite the current snapshot SHA-256 for past competences; region labels
   are not wired from a catalogue.
5. The backfill port's build path was reviewed, not re-run (it needs a full monthly
   download and rebuild).
