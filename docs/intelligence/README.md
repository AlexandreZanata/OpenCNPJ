# OpenCNPJ Intelligence — plan for a lightweight pt-BR data assistant

Requirements, research, design and staged execution plan for a conversational assistant
that answers managers' questions about Brazilian companies **in milliseconds**, from the
real OpenCNPJ data, **without LLMs, generative models or external AI services**, on
hardware as small as **1 vCPU / 1 GB RAM**.

Status (2026-11-07): phases 0–3 of [14-ROADMAP.md](14-ROADMAP.md) are implemented
(catalogues, aggregates, query IR, validator, planner/executors, renderer, generator,
evaluation harness) and were reviewed — see
[results/2026-11-07-review.md](results/2026-11-07-review.md) for what was fixed, what is
still open and the status of each work package, and
[results/2026-11-07-reviewer-tasks.md](results/2026-11-07-reviewer-tasks.md) for the
follow-up round (independent differential, no Python, reviewer pass, new evaluation items).
The deterministic interpreter now has an implementation in `opencnpj-nlu`, with
`opencnpj-intel ask` and `eval-questions`, a 1,000-question synthetic commercial
round, real-data execution and separate frozen-suite results. See the
[commercial report](results/commercial-round-01/REPORT.md) and
[reproduction runbook](results/commercial-round-01/RUNBOOK.md).
This starts phase 4; it does not complete its architecture, calibration, compiled-pack,
transport or resource gates. The owner explicitly deferred human question collection
and human evaluation to product refinement; they do not block this synthetic
iteration. Existing owner confirmations remain recorded as pending. The snapshot
format and production store engine remain unchanged.

## How it works (one paragraph)

A question in Portuguese is normalized, its places, sectors, sizes, statuses, numbers and
dates are recognized from official catalogues (IBGE, Receita Federal, a curated sector
catalogue) and replaced by typed placeholders; compiled grammar constructions — and, only
if it earns its place, a tiny linear classifier — decide what is being asked; the result
is a typed, validated query (IR). The IR runs on small precomputed aggregates (counts,
rankings, densities, time series) or on the existing OpenCNPJ engine (lists, lookups),
and the answer is rendered from templates that cite the data date, the CNAE codes used
and the population source. Unclear questions get one short clarifying question; questions
outside the domain are declined.

## Documents

| # | Document | Content |
|---|---|---|
| 01 | [REQUIREMENTS](01-REQUIREMENTS.md) | Users, scope, functional/semantic/conversational/non-functional requirements, hardware baseline, definition of done, owner decisions |
| 02 | [AUDIT](02-AUDIT.md) | What OpenCNPJ already provides, data dimensions, engine latencies, gaps |
| 03 | [RESEARCH-AND-OPTIONS](03-RESEARCH-AND-OPTIONS.md) | Published evidence, approaches A/B/C/D/E, comparison matrix, the brief's §12 questions as hypotheses, gates |
| 04 | [QUERY-IR](04-QUERY-IR.md) | Typed intermediate query language in Rust, validation, canonicalization, cache keys, planning |
| 05 | [INTERPRETER](05-INTERPRETER.md) | Deterministic interpreter: normalization, language pack, entity spotting and fuzzy matching, time grammar, constructions |
| 06 | [CONTEXT](06-CONTEXT.md) | Conversation state (< 256 B), dialogue acts, inheritance rules, scripted dialogues |
| 07 | [CONFIDENCE-AND-SAFETY](07-CONFIDENCE-AND-SAFETY.md) | Selective answering, signals, policy, calibration, ambiguity, negation, data protection |
| 08 | [RESPONSES](08-RESPONSES.md) | Answer envelope, blocks, templates, deterministic variation, caveat catalogue |
| 09 | [DATA-ASSETS](09-DATA-ASSETS.md) | External sources (verified URLs), municipality and sector catalogues, `.ocagg` aggregates, history |
| 10 | [SYNTHETIC-DATA](10-SYNTHETIC-DATA.md) | Generator, linguistic variation and noise, generalization splits, human and adversarial sets |
| 11 | [MICRO-CLASSIFIER](11-MICRO-CLASSIFIER.md) | Optional linear classifier (and gated neural option), compact artifact, calibration |
| 12 | [BENCHMARK-PROTOCOL](12-BENCHMARK-PROTOCOL.md) | Metrics, latency and memory methodology, 1 vCPU emulation, learning curves, thresholds |
| 13 | [RESOURCE-ESTIMATES](13-RESOURCE-ESTIMATES.md) | RAM, CPU, storage and cost estimates with their basis |
| 14 | [ROADMAP](14-ROADMAP.md) | **Execution plan**: phases, work packages, acceptance criteria, rules for the executing agent |
| 15 | [DECISIONS](15-DECISIONS.md) | Decision log (ADR) and justification of the recommended architecture |
| — | [results/](results/) | Measured results with commands and raw output |

## The brief's deliverables (§15) and where they are

| Deliverable | Where | State |
|---|---|---|
| 1. What can be reused from OpenCNPJ | 02-AUDIT | Done |
| 2. Comparison matrix of architectures | 03 §4 | Done (priors; replaced by measurements in WP-5.1) |
| 3. Intermediate query language | 04 | Done (design) |
| 4. Synthetic pt-BR data strategy | 10 | Done (design) |
| 5. Deterministic interpreter design | 05 (+ 06, 07) | Done (design) |
| 6. Micro classifier proposal, only if needed | 11 + gates in 03 §6 | Done (conditional design) |
| 7. Reproducible benchmark protocol | 12 | Done |
| 8. Measured results of the proofs of concept | results/ | Aggregate and interpreter development evidence; see [round-08 review](results/commercial-round-08/REPORT.md) |
| 9. RAM, CPU and storage estimate | 13 | Done (estimates, labelled) |
| 10. Development plan in independent stages | 14 | Done |
| 11. Technical justification | 15 | Done (provisional; final after WP-5.1) |

## What is measured, estimated or hypothetical

* **Measured (2026-11-04):** aggregate cardinalities on the 2026-10 national data
  ([results/2026-11-04-aggregate-cardinality.md](results/2026-11-04-aggregate-cardinality.md));
  reference-table facts (5,572 municipalities with no UF column and 241 homonym names;
  1,359 CNAE subclasses); build-host hardware and free disk; engine latencies (from
  `docs/BENCHMARKS.md`); availability of RFB history (2023-05 … 2026-10) and of the IBGE,
  Tesouro and CNAE sources (web checks listed in 09 §8).
* **Estimated:** aggregate file sizes, memory and storage budgets (13), with formulas.
* **Measured (2026-11-07):** synthetic commercial interpretation/execution and older
  frozen-suite scores in the commercial report.
* **Reviewed (2026-11-08):** [round 08](results/commercial-round-08/REPORT.md)
  distinguishes frozen-label agreement, confirmed label defects, backend
  verification and focused semantic regressions. Synthetic scores do not
  establish product validation.
* **Hypotheses:** generalization to human product usage and interpretation latency on
  the target VPS. Architecture gates still require phases 4–5.

## Guiding principles

1. Correctness before coverage: ask or decline rather than answer wrongly.
2. Every number comes from a query; every claim respects the data's semantics (SR rules).
3. The simplest mechanism that meets the targets wins; learned components must prove
   their value on human-written questions.
4. Everything is data-driven, versioned, reproducible and reversible.

## Next evaluation round

[Round 09 expansion](ROUND-09-EXPANSION.md) defines 50,000 commercial cases,
MT-first scenarios, sourced regional language and analytical capability boundaries.
See the [handoff prompt](ROUND-09-PROMPT.txt) and
[versioning policy](EVALUATION-VERSIONING.md) for execution and GitHub stages.
