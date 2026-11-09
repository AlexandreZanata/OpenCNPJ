# 15 — Decision log and justification

Architecture decisions for the assistant, in ADR form. Status: **Accepted** (constraint or
settled by evidence), **Proposed** (recommended; the owner may change it), **Pending**
(to be decided by measurements in WP-5.1).

## ADR-I-001 No LLM, generative model or external AI — Accepted (constraint)

The brief forbids generic LLMs, generative models, external or paid AI APIs in
production, and heavy runtimes. Transformers, generic embeddings and vector databases are
excluded unless measurements show the selected approach cannot meet NFR-07/08 (and even
then they would violate NFR-04/05/10; 03-RESEARCH-AND-OPTIONS.md §3 E5).

## ADR-I-002 A typed, validated IR is the only execution contract — Accepted

Every interpreter outputs `opencnpj-query` IR; only `ValidRequest` values (constructible
only by the validator) reach a backend. Reasons: safety (no free-form execution),
paraphrase equivalence via canonicalization, stable cache keys, reproducible answers,
and a single place where data semantics (SR rules) are enforced.

## ADR-I-003 Entities are resolved from catalogues before intent — Accepted (variant to measure)

Municipalities, sectors and codes are looked up in versioned catalogues; the intent
layer sees typed placeholders. Reasons: the model never needs to memorize 5,572
municipalities or CNAE codes (brief §4); swapping Sinop for any other municipality cannot
change the intent; catalogues are auditable. Whether entity alternatives should be kept
as a lattice (A2) or resolved early (A1) is measured in WP-4.1.

## ADR-I-004 Local aggregates as a separate derived artifact — Accepted

National rankings and densities on the engine take 20–200 ms (and 2–3 s for secondary
CNAE) per query on a big machine (02-AUDIT.md §4), too slow for a 1 vCPU node. Measured
cardinalities (2.83 M cells for the active stock table; 8.7 M / 5.4 M monthly flow cells
since 2021) show the aggregates fit in tens of MB per competence. They live in their own
`.ocagg` file built from a verified snapshot, so the `.ocnpj` format and FORMAT.md stay
untouched, and a differential test against the engine guards correctness (NFR-12).

## ADR-I-005 Stateless signed context token — Proposed

Conversation state (< 256 bytes) travels with the client as an HMAC-signed token
(06-CONTEXT.md §1). Reasons: no per-session memory on a 1 GB node, no sticky routing,
survives the existing zero-downtime upgrades. Alternative kept: server-side TTL map.

## ADR-I-006 Four new crates in the existing workspace — Proposed

`opencnpj-query`, `opencnpj-nlu`, `opencnpj-insights`, `opencnpj-lab`
(14-ROADMAP.md §1). Reasons: one lockfile and delivery gate, reuse of core
normalization/CNPJ code, language crates free of storage and HTTP dependencies (reusable
by other Brazilian projects), research code isolated from the product.

## ADR-I-007 Interpreter architecture — Pending (WP-5.1)

Provisional hypothesis: deterministic interpreter with an entity lattice (A2), plus a
logistic-regression intent/act scorer on delexicalized input only if gate G-B passes
(hybrid D). Neural model only through gate G-C. This entry is rewritten with the measured
comparison when WP-5.1 is done.

## ADR-I-008 Two-node topology with a single-node mode — Proposed

A 1 vCPU / 1 GB chat node holds the language pack, catalogues and aggregates; a data node
runs the existing `opencnpj serve` with the national snapshot for lists, lookups and rare
filters (13-RESOURCE-ESTIMATES.md §1). The same code runs in one process for development
and small deployments. Reason: the snapshot needs 8–16 cores and 16 GB to serve heavy
queries (DEPLOYMENT.md), while most managerial questions are aggregate questions.

## ADR-I-009 What goes into Git — Proposed

Hand-edited data (sector catalogue, aliases, cues, constructions, templates, overrides)
and small generated catalogues (< 2 MB each) are committed with source manifests.
Aggregates, raw downloads, synthetic datasets and classifier weights are not; they are
rebuilt reproducibly and referenced by SHA-256 (09-DATA-ASSETS.md §6).

## ADR-I-010 Semantic defaults — Proposed (owner decisions D-01 … D-09)

Listed with recommendations in 01-REQUIREMENTS.md §10. Implemented as data/config so a
change of mind does not require code changes.

---

## Justification of the recommended architecture (current state of evidence)

What is **measured** today: the engine's latencies (BENCHMARKS.md), the aggregate
cardinalities, the reference-table facts (municipality homonyms, missing UF column, CNAE
code counts), the build host's constraints, and the availability of the external sources
(09-DATA-ASSETS.md). What is **not yet measured**: anything about language understanding
quality or interpreter latency on our task.

Given that, the recommendation is the least-resource design that can meet the
requirements, structured so the evidence can overturn it cheaply:

1. **The hard parts are closed-world lookups and abstention, not "intelligence".** Entities
   come from finite official catalogues; the output space is a small typed IR; the safety
   requirement (≥ 99 % accepted precision) rewards explicit rules and a strict policy.
   Published embedded systems (Snips) reached millisecond latency on laptops with a
   deterministic first stage and linear models — and our domain is narrower.
2. **Data speed comes from aggregates, not from the interpreter.** Interpretation is
   microseconds in all candidate designs; the expensive part is national group-bys, which
   the measured cardinalities show can be precomputed into tens of MB.
3. **Every learned component is optional and gated.** The classifier only ships if it
   adds ≥ 5 pp coverage on the blind human set without losing precision, within 2 MB and
   0.5 ms; the neural model only if the linear one demonstrably cannot represent the
   failures. Both have the rules as an always-available fallback.
4. **Correctness is enforced outside the language layer.** The validator, the
   differential aggregate tests and the caveat catalogue make it impossible for any
   interpreter variant to produce an unsupported number or an overstated claim.
