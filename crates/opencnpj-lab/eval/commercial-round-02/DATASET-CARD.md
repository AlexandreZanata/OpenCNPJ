# Commercial research questions — round 02

This is a **synthetic development set**, authored by an AI for the commercial
research workflows in `docs/intelligence/01-REQUIREMENTS.md`. These are plausible
customer questions, not collected customer conversations or human annotations.
Human evaluation is deferred to product refinement, as requested by the owner.
No human approval is required to execute this synthetic development round.
Round 01 stays frozen; this round asks genuinely new questions with
independently authored labels.

## Composition

Exactly 1,000 UTF-8 question records: 895 executable
requests, 50 clarifications and 55
rejections. There are 20 equally weighted categories of 50 questions:
market sizing, prospecting, tax/size, structure/status, capital/legal nature,
registrations, closures, trends, historical stock/growth, geography, density,
comparisons, distributions/rankings, activity/CNAE, exclusions, lookup,
methodology, dialogue, clarification, and unsupported requests.

The executable labels use all 213 current curated sector IDs. The geographic
scenarios include all 27 UFs and 60 municipalities. Questions include
main/secondary CNAE, all operation variants, all status/porte classes, MEI and
Simples, headquarters/branches, capital and legal nature, registration/closure
dates, geography/population, rank direction, list ordering/pagination and
contact-presence filters.

There are ten five-turn sessions. `context.session_id` groups them;
`previous_ir` is an annotation for inspection, **never an interpreter input**.
The runner carries only the interpreter's own state between turns.

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
