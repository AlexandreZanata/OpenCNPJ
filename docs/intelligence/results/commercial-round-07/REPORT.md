# Commercial research round 07 — 20,000 questions, same loop, measured results

Date: 2026-11-07. Data anchor: 2026-10-11; competence 2026-10.
All measurements below come from the commands and complete raw output in
[COMMANDS.md](COMMANDS.md). No latency, RSS or target-VPS claim is made.

## Outcome

Round 07 contains exactly **20,000 new synthetic commercial questions** in 20
categories of 1,000: 17,900 executable requests, 1,000 clarifications and 1,100
rejections. It is built from two deterministic panels of 10,000 with different
catalogue index shifts, city orders, OOD slices, historical month pairs, rank
sizes, CNPJ roots and session IDs. Every decision and typed expected IR was
authored independently of the interpreter's predictions: labels come from the
catalogue data, the source rules and calendar arithmetic, never from a
prediction.

The historical growth pairs, rank sizes, out-of-domain slices and CNPJ roots are
disjoint from round 06. The 20,000 normalized texts are distinct. Overlap with
rounds 01-06 is limited to the documented exemptions: 270 dialogue turns 2-4 and
four fixed small-talk sentences (274 items). 19,010 items needed no filler
repair; 990 distinct items received a filler-only repair because a template
collision remained (508 prior-round collisions, 443 cross-panel collisions, 230
within-panel collisions; the sets overlap). Those repairs change only filler
words, never a slot, filter or gold IR. This is a synthetic development set, not
a blind or human validation set.

## National execution

- **19,954/20,000** matches against this round's synthetic labels (99.77 %).
- **17,900/17,900 executable IRs** returned successfully; **0 invalid gold
  labels** and **0 out-of-policy executions**.
- **16,900/16,900** labelled dialogue acts matched.
- **11,140 current aggregate/engine comparisons** matched exactly.
- **700 historical comparisons** matched independent marginal sums; **300**
  have no independent marginal reference (single-artifact results, retained as
  data outcomes); **1,000** methodology/meta results are non-numeric.
- **4,760 requests** used a single engine path. They succeeded, but are not
  independent differential proofs.
- **2,166 zero scalar results** and **406 empty record searches** were retained
  as legitimate data outcomes.
- No backend error, differential mismatch, wrong-answer execution or dropped
  row was hidden or converted to success.

For executable questions, a match is equality of canonical typed requests,
including all explicit filters. For clarification/rejection cases it is the
expected decision, not a proof that every explanation or option is ideal.
Historical marginal checks use the same archived data through separate logic;
they are not a comparison with a separately available old engine snapshot.

The runner receives only text plus its own prior predictions. It never reads
`gold.ir` or `context.previous_ir` as interpreter input. Labels are used after
prediction for scoring and for a separate backend audit. Both predicted and
expected execution results are retained in `national/results.jsonl`.

Dataset SHA-256:
`a428268809cea38917247fa929c109d84da9339139f561b478bc18efaa6217af`.
Snapshot SHA-256, recomputed and checked against the aggregate manifest:
`76f36078c1e5b727af8416262309d1285e39f054514218a75a46206d5786037b`
(same local files as rounds 01-06). The dataset manifest pins question, export
and source-catalogue hashes.

## Changes driven by the evaluation

The first scored pass had 47 mismatches. Two were general defects exposed by new
round-07 combinations, not label errors:

- **"Não" inside the municipality name.** `Não-Me-Toque/RS` was treated as a
  negation, so both a count and an out-of-scope how-to question became a
  clarification instead of executing/rejecting. Words inside recognized entity
  spans are now skipped by the unsupported-filter/negation scan.
- **Number words in a multi-word place name.** The place-typo item `Três
  LLagoas/MS` was accepted as `MS` because `três` was treated as a numeric
  quantity, bypassing the multi-word place fail-closed check. The check now
  skips only ASCII-digit quantities, while a legitimate `Três Lagoas/MS` still
  resolves through the entity span.

Both fixes are general interpreter fixes with focused regressions in
`crates/opencnpj-nlu/src/tests.rs`; no gold label was changed to fit a
prediction. The round-06 fixes (CNPJ trailing punctuation, `serraria`
homonym/activity context, entity-aware intent cues, multi-word place fail-closed)
remain in force and showed no regression.

## Leftover analysis: the 46 mismatches

All 46 remaining mismatches are `place_typo` stress items in `market_size`
(city name with one duplicated letter). Every one is predicted as `clarify`;
there is no wrong-answer execution. The 120 place-typo items split as follows:

- 74 recovered to the labelled municipality;
- 46 failed closed: 30 multi-token names and 16 single-token names whose
  duplicated-letter form has more than one one-edit catalogue candidate.

Examples of the retained leftovers:

- `Três LLagoas/MS` -> clarify (multi-token name);
- `Maraabá/PA` -> clarify (single token with an ambiguous fuzzy candidate);
- `Salvaador/BA` -> clarify (single token with an ambiguous fuzzy candidate);
- `Santtos/SP` -> clarify (single token with an ambiguous fuzzy candidate).

The rule is deliberate: widening recovery to every token would accept wrong
municipalities in multi-word names. The bound is documented and kept visible; no
code change was made to fit these labels. Human refinement must decide whether
to offer a disambiguation list for multi-word place typos. All other 19,954
items match, including the new historical month pairs, new rank sizes, new CNPJ
roots, panel-specific OOD slices and dialogue sessions 301-500.

## Cross-check against previously frozen suites

These inputs were not rewritten. They were run through actual text
interpretation with the same canonical comparator and declared archives.

- **frozen-adv: 198/304** canonical/decision matches; 106 disagreements, 12
  executions against a clarify/reject label.
- **frozen-informal: 116/202**; 86 disagreements, 2 executions against a
  clarify/reject label.
- **frozen-dialogues: 81/122**; 41 disagreements, 0 executions against a
  clarify/reject label.
- **frozen-ood: 377/400**; 23 disagreements, 0 executions against a
  clarify/reject label.

Earlier commercial rounds replayed with the final binary (`--interpret-only`,
outputs outside the repository): round 01 1000/1000, round 02 1000/1000, round
03 2994/3000, round 04 4987/5000, round 05 9975/10000, round 06 19954/20000. No
prior-round score regressed. The lower ADV, informal and dialogue scores still
show that a strong development-round score is **not** universal Portuguese
coverage or readiness for product validation.

## Delivery checks and remaining scope

`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D
warnings` and `cargo test --workspace` exited successfully. The raw test output
records **294 passed, 0 failed, 5 ignored**. See `fmt.log`, `clippy.log` and
`test.log`.

This is a working deterministic implementation and repeatable evaluation loop,
with measured development coverage. It does **not** complete every phase-4/5
architecture gate. Finite examples cannot prove every phrasing or filter
combination. The place-typo bound, the frozen-suite disagreements (unqualified
place names, informal count/list readings, sector-vs-CNAE representations, rank
defaults, short references, pre-archive labels), confidence calibration,
compiled language packs, signed HTTP context transport, application/API
integration and target-hardware measurements remain outside this delivery. The
CLI's state file is local state.

Human question collection and human evaluation are explicitly deferred to
product refinement, as requested. No human approval was required to execute this
synthetic round. H-blind was neither opened nor run. Existing source datasets
were not modified; snapshot format and production store code remain unchanged.
No commit or push was made.

## Per-category evidence

- `activity`: 1000/1000 interpretation matches; 1000 executable labels returned successfully.
- `capital_legal`: 1000/1000; 1000 executable labels returned successfully.
- `clarification`: 1000/1000; 0 executable labels returned successfully.
- `closures`: 1000/1000; 1000 executable labels returned successfully.
- `comparison`: 1000/1000; 1000 executable labels returned successfully.
- `density`: 1000/1000; 1000 executable labels returned successfully.
- `dialogue`: 1000/1000; 1000 executable labels returned successfully.
- `distribution`: 1000/1000; 1000 executable labels returned successfully.
- `exclusion`: 1000/1000; 1000 executable labels returned successfully.
- `geography`: 1000/1000; 1000 executable labels returned successfully.
- `historical`: 1000/1000; 1000 executable labels returned successfully.
- `lookup`: 1000/1000; 900 executable labels returned successfully (the 100 invalid check digits correctly clarify).
- `market_size`: 954/1000; 1000 executable labels returned successfully; the 46 mismatches are the place-typo leftovers above.
- `methodology`: 1000/1000; 1000 executable labels returned successfully.
- `prospecting`: 1000/1000; 1000 executable labels returned successfully.
- `registrations`: 1000/1000; 1000 executable labels returned successfully.
- `structure_status`: 1000/1000; 1000 executable labels returned successfully.
- `tax_size`: 1000/1000; 1000 executable labels returned successfully.
- `trends`: 1000/1000; 1000 executable labels returned successfully.
- `unsupported`: 1000/1000; 0 executable labels returned successfully.
