# Round 08 completed and semantic defects reviewed

Review date: 2026-11-08. Data anchor: 2026-10-11; competence 2026-10.
Commands and raw evidence: [COMMANDS.md](COMMANDS.md). Machine-readable
completion: [index.json](index.json). No commit or push was made.

## Outcome

- The original national run completed all **50,000/50,000** questions: **49,864/50,000** original label matches.
- Every text was reinterpreted with the corrected final binary: **48,264/50,000** matches against the unchanged labels.
- **1,600 original closure labels are semantically defective.** They treat a start-of-activity period as the closure period and omit the start-date filter. The corrected parser clarifies instead of answering that different question.
- The other **136 disagreements** are municipality-typo questions that clarify. No final mismatch emits an executable request.
- The separate, hash-bound semantic adjudication patch yields **49,864/50,000** agreement. This is a secondary development-set score, not blind or human validation.
- **20/20 focused review cases** matched; all **13 executable review requests** succeeded on the actual local artifacts.
- Final delivery gate: **300 passed, 0 failed, 5 ignored**; formatting and Clippy passed.

The lower raw frozen-label score is explained by retaining the 1,600 bad
labels for audit. They were not silently changed to fit predictions.
`label-adjudications.jsonl` records replacement decisions, original-gold
hashes, authorship and semantic reasons separately. Human validation stays
deferred to product refinement, as requested. H-blind was not opened or run.

## Corrections made

1. Date literals retain separators while entity names are masked. Dates no longer become eight-digit CNPJ roots; invalid calendar dates clarify.
2. Historical stock needs an explicit monthly competence. A year does not silently choose December; years with no available snapshots are rejected.
3. Closure periods and start-of-activity filters are resolved separately. A missing closure period clarifies; explicit periods retain both constraints.
4. Start-date filters survive on lists and stock counts.
5. Informal size words such as grandonas clarify instead of being discarded.
6. Numeric capital/population filters do not become identifier lookups. Explicit and standalone CNPJ roots remain supported.

Focused regression tests exercise these behaviors. Previously implemented
entity-name fixes remain in place, including Contagem and Não-Me-Toque.
The new review dialogue covers both cities without editing the old set.

## Independent frozen suites

- Adversarial: **198 -> 210 / 304**.
- Informal: **116 -> 118 / 202**.
- Dialogues: **81 -> 81 / 122**.
- Out of domain: **377 -> 377 / 400**.

All 14 previously emitted executable requests against clarify/reject
labels now clarify. The legacy counter is called out_of_policy_execution,
but in interpret-only runs it counts executable requests, not actual
backend calls. Improvements are real but limited; the unchanged dialogue
and out-of-domain scores still show substantial language gaps.

Prior commercial rounds were also replayed with the final binary. Their
raw agreement falls where the same closure-label defect exists; see the
per-round counts and label-defect audit in index.json. Round 01 remains
1000/1000. A repeated high template score is not universal Portuguese coverage.

## Backend evidence and completion integrity

- The baseline executed **44,750 typed reference requests** successfully. Well-typed reference IR does not prove its text label is semantically correct.
- The finalizer checked **43,014 accepted final requests** against exactly equal typed baseline requests with successful executions.
- Existing executions were reused only for equal IR on the same pinned data. No second full national backend run with the corrected parser is claimed.
- The 20 new review cases were executed separately, rather than borrowing results from different filters.
- Every original input has exactly one baseline result and one final interpretation, in order, with matching ID and text. No input record was dropped.
- Original input/export/catalogue hashes match their manifest; final source and evidence hashes are retained.

Baseline differential evidence:

- verification_engine_match: 27,850.
- verification_marginal_match: 1,750.
- verification_no_independent_reference: 750.
- verification_not_numeric: 2,500.
- verification_single_backend: 11,900.

Single-backend and missing-reference results are not independent
differential proofs. No throughput, latency, memory or target-VPS claim is made.

## Dataset and remaining adjustments

- The 50,000 texts are distinct within this set. There are 363 earlier-text overlaps: 359 dialogue turns and four fixed small-talk questions. Repeated follow-ups may have different contexts.
- 200 session names are reused from round 07. Dataset evaluations reset state independently; this is a documentation/provenance defect, not shared conversation input.
- The card now correctly states 45,911 items without filler repair, not 15,911. Filler repairs do not establish semantic independence.
- Next label revision should apply and review the explicit adjudication patch while retaining the frozen original as evidence.
- Prioritize the remaining adversarial/informal/dialogue failure families, semantically varied questions and supported clarification options. Do not force execution of ambiguous place typos to improve a score.
- Durable evaluator checkpoints/resume, confidence calibration and application/API/context integration remain product work. Human refinement and validation remain later stages.

This round is complete. Product readiness and coverage of every possible
commercial phrasing are not established by these synthetic evaluations.

## Execution handling

The original national process was preserved through completion. Automatic
approval review rejected four parallel backend shards with process suspension,
and later rejected another concurrent evaluation batch, citing resource and
interruption risks. Neither rejected action ran. Final re-evaluations ran
sequentially only after the original national executable exited.

## Short handoff prompt

> Read AGENTS.md and commercial-round-08/{REPORT,COMMANDS,index}. Preserve
> frozen inputs; use the explicit label-adjudication patch as a separate
> version. Improve remaining independent-suite failures with focused
> semantic cases, retain ambiguous-place clarifications, rerun the gate
> and record before/after evidence. Avoid another large template round.
> Keep H-blind intact, defer human validation, and do not commit or push.
