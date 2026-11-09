# Commercial research questions — round 06

This is a **synthetic development set**, authored by an AI for the commercial
research workflows in `docs/intelligence/01-REQUIREMENTS.md`. These are plausible
customer questions, not collected customer conversations or human annotations.
Human evaluation is deferred to product refinement, as requested by the owner.
No human approval is required to execute this synthetic development round.
Rounds 01-05 stay frozen; this round asks genuinely new questions with
independently authored labels. Outside the exempt dialogue turns 2-4 and four
fixed small-talk sentences, there is no normalized-text overlap with them.

## Composition

Exactly 20,000 UTF-8 question records: 17900 executable
requests, 1000 clarifications and 1100
rejections. There are 20 equally weighted categories of 1,000 questions:
market sizing, prospecting, tax/size, structure/status, capital/legal nature,
registrations, closures, trends, historical stock/growth, geography, density,
comparisons, distributions/rankings, activity/CNAE, exclusions, lookup,
methodology, dialogue, clarification, and unsupported requests.

The set is built from two disjoint deterministic panels of 10,000 (different
catalogue index shifts and city order, different OOD slices, different
historical month pairs, different rank sizes and different CNPJ roots). Each
panel keeps the validated round-05 scenario families; the two panels therefore
cover new sector/city/period/filter combinations, not only repeated sentences.
Of the 20,000 texts, 567 were prior-round template collisions,
476 collided across panels and 208 needed within-panel
disambiguation; 998 distinct items received at least one filler-only
repair (`overlap_repair`, `cross_panel_repair`, `dedup_filler`) and
19002 needed none. No slot, filter or gold IR was touched.

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

There are two hundred five-turn sessions (100 per panel). `context.session_id`
groups them; `previous_ir` is an annotation for inspection, **never an
interpreter input**. The runner carries only the interpreter's own state between
turns. Contagem/MG is intentionally excluded from city-qualified dialogue turns
(the documented cue-collision limitation from round 05); the rest of the place
coverage is unchanged.

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
