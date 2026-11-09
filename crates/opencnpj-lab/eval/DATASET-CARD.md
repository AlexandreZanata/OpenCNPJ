# Evaluation sets — ADV, OOD, informal and dialogues

First frozen on 2026-11-06 by a generator script that was removed on 2026-11-07: the
JSONL files are now the source of truth and are edited by hand. Gold IRs are labelled
against the committed catalogues (`opencnpj-insights/catalog/municipalities.tsv` and
`sectors.toml`, core CNAE sets); the frozen-set test validates every gold against them.
After editing a file, refresh the rows below with
`UPDATE_CARD=1 cargo test -p opencnpj-lab frozen_sets`.

Status: AI-drafted. Items with `meta.origin = "claude-review-draft"` were written by hand
by the reviewing model on 2026-11-07 (not by the generator or its templates); the rest
come from the removed script. Sizes and rule/trap coverage now meet WP-3.3
(`wp_3_3_acceptance`), but WP-3.3 also requires sets written by people who never saw the
generator or the grammar, so these sets must still not be used for the WP-5.1 decision.
See results/2026-11-07-review.md and results/2026-11-07-reviewer-tasks.md.

| File | Items | SHA-256 |
|---|---|---|
| adv.jsonl | 304 | 3d714467537db22f20855ce148f010234fe72eb78a31cbdb623733eb703a5c0b |
| ood.jsonl | 400 | c3eafa0206e53cd908de5ee71d81a5f7a6de4015f35c11b6036af207f158eb92 |
| h-informal.jsonl | 202 | 13a32a64716d91c0de8d79823d1e1328b5792d68b3207a90bfac99c3d6f1be7c |
| dialogues.jsonl | 60 | 58e3a50e851ba5b6d01e5a4e29f582f52cd69580294ca518be466bb62838f5cb |

## Schemas

* `adv.jsonl`, `ood.jsonl`, `h-informal.jsonl`: one `opencnpj_query::ir::Request`
  gold per executable item, or a clarify/reject decision with a rationale.
* `dialogues.jsonl`: `{"id","turns":[{"text","gold"}...],"meta"}`; the gold
  after each turn is a complete IR or a decision.

## Coverage

Traps of 05-INTERPRETER.md §7: abertas=96, crescendo=25, fecharam=25, grandes=4, mais=2, menos=2.
Semantic rules with at least five items: SR-01=80, SR-02=96, SR-03=16, SR-04=25, SR-05=25, SR-06=18, SR-07=9, SR-08=16, SR-09=9, SR-10=10, SR-11=5, SR-12=4, SR-13=13, SR-14=5.

## Sources and attribution

The OOD set is original for this evaluation. The planned import of
`OHF-Voice/intents` pt-BR (CC BY 4.0) was attempted through the Hugging Face
datasets server on 2026-11-06; the dataset is gated and the request returned an
authentication error, so **no rows were imported**. If the import is retried,
retain the CC BY 4.0 attribution and add the rows to a separate section of this
card; do not mix them into H-blind.

## Usage

`opencnpj-intel eval --set eval/adv.jsonl --predictions ...` (or the baseline)
and the schema test in `opencnpj-lab`. The H-blind guard does not apply to these
sets.
