# Commercial research questions — round 08

This is a **synthetic development set**, authored by an AI for the commercial
research workflows in `docs/intelligence/01-REQUIREMENTS.md`. These are plausible
customer questions, not collected customer conversations or human annotations.
Human evaluation is deferred to product refinement, as requested by the owner.
No human approval is required to execute this synthetic development round.
Rounds 01-07 and this round's original questions stay frozen. This round varies
the previous scenario templates; it is not an independent language validation
set. The review found 363 text overlaps with earlier rounds: 359 short dialogue
turns and four fixed small-talk questions. Different dialogue contexts can make
identical follow-up text legitimate; overlap is disclosed rather than repaired
by adding filler.

## Composition

Exactly 50,000 UTF-8 question records: 44750 executable
requests, 2500 clarifications and 2750
rejections. There are 20 equally weighted categories of 2,500 questions:
market sizing, prospecting, tax/size, structure/status, capital/legal nature,
registrations, closures, trends, historical stock/growth, geography, density,
comparisons, distributions/rankings, activity/CNAE, exclusions, lookup,
methodology, dialogue, clarification, and unsupported requests.

The set is built from five disjoint deterministic panels of 10,000 (different
catalogue index shifts and city order, different OOD slices, different
historical month pairs, different rank sizes and different CNPJ roots). Each
panel keeps the validated round-07 scenario families; the panels therefore
cover new sector/city/period/filter combinations, not only repeated sentences.
Session IDs 301-500 are reused from round 07; each dataset starts a separate
conversation state, so the names do not share runtime context. The other
claimed panel differences are scenario variations, not proof of independence.
Of the 50,000 texts, 2868 were prior-round template collisions,
2048 collided across panels and 518 needed within-panel
disambiguation; 4089 distinct items received at least one filler-only
repair (`overlap_repair`, `cross_panel_repair`, `dedup_filler`) and
45,911 needed none. No slot, filter or gold IR was touched.

The executable labels use the current curated sector IDs. The geographic
scenarios include all 27 UFs and 250 main-pool municipalities per panel, plus
disjoint out-of-domain slices. Questions include main/secondary CNAE, all
operation variants, all status/porte classes, MEI and Simples (alone and
combined), headquarters/branches, capital and legal nature (alone and
combined), registration/closure dates, geography/population, rank direction and
size, list ordering/pagination and contact-presence filters. Existing shapes
include status-date month windows, strict capital and population bounds,
national ranks, three-UF comparisons, per-10k comparisons, no-activity counts,
deictic national scope, sector-replacement and capital dialogue turns, vague
quantity and how-to rejections, and a noise slice (unaccented, abbreviated,
light typo, place typo, all caps, extra punctuation).

There are five hundred five-turn sessions (100 per panel; session IDs 301-800). `context.session_id`
groups them; `previous_ir` is an annotation for inspection, **never an
interpreter input**. The runner carries only the interpreter's own state between
turns. The original generation excludes Contagem/MG from city-qualified
dialogue turns even though the interpreter now has a regression for its cue
collision. The separate `review-regressions.jsonl` exercises that municipality
and does not change the original 50,000 questions.

## Review caveat

The 2026-11-08 audit identifies **1,600 closure labels** that assign a stated
registration/start-of-activity period to the closure date and omit the
registration-date filter. The corrected interpreter asks for the missing
closure period. Original labels remain unchanged: report raw label agreement
and this defect count separately. These disagreements must not be presented
as a language regression or silently relabelled to improve the score.

`review-regressions.jsonl` has 20 independently authored review cases covering
full dates, invalid dates, historical-month ambiguity, registration versus
closure dates, date-filtered lists/counts, informal size ambiguity and entity
names in dialogue. Its manifest is separate from the frozen original set.

## Format and provenance

- `questions.jsonl` is the frozen, directly editable source of truth.
- `questions.tsv` is a reading/export view, with one row per question.
- `manifest.json` pins hashes of the questions and source catalogues.
- Every row has an ID, question, expected decision, expected typed IR when
  executable, rule tags, category, rationale and explicit synthetic authorship.
- Language is pt-BR. The data reference is **2026-10-11**, competence **2026-10**.
- Monetary IR values are integer cents. Population bounds and dates are inclusive.
- Sector IDs follow the checked-in `sectors.toml` file order, not display names.

Scenario formulations and catalogue substitutions were assembled with a
one-off local authoring script. No third-party question dataset was imported.
The checked-in RFB/IBGE-derived catalogues supply vocabulary and identifiers;
their existing source manifests and licences remain authoritative. There is no
new Python dependency in the repository, interpreter, evaluator or replay flow.
