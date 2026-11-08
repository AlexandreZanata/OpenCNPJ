# Work order: official-data integration, MT first

Date: 2026-11-08. Owner requests phased implementation with tests, commits and
GitHub pushes. This document authorizes the next AI's implementation workflow;
it does not claim any new source has already been integrated.

## Objective and execution authority

Deliver an end-to-end, source-backed commercial-research capability starting
with the essential information and the Mato Grosso pilot, followed by the
remaining researched sources. Build usable vertical slices, not only downloaded
files, mock answers or a new question generator. Human product validation remains
deferred to refinement. Do not open or run the existing H-blind set.

Read AGENTS.md, COMMIT_CONVENTION.md, DATA_POLICY.md, intelligence requirements,
09-DATA-ASSETS.md, AGGREGATES-FORMAT.md, OFFICIAL-DATA-EXPANSION-2026-11-08.md,
round-08 results, and the actually completed round-09 evidence if available.
Recheck source endpoints, current editions, metadata and resource licences at
implementation time. Research URLs are discovery entry points, not frozen API
contracts or promises that a source is already connected.

The owner authorizes incremental scoped commits and pushes for this work.
Continue through available stages without asking for approval after every
milestone. Do not merge, force-push, purchase services or commit unrelated work.
If a source genuinely requires unavailable access/rights, report the exact
external dependency and continue independent stages. Do not call that component
complete or replace it with simulated production data.

## Stage 0 — Isolate work and establish a measured baseline

The working tree observed on 2026-11-08 is on codex/commercial-round-09 with
uncommitted NLU/evaluator changes. This is context, not an assumption that remains
true: inspect the current state before acting.

Tasks:
- Record branch, HEAD, status, remotes, worktrees and active heavy evaluators.
- Create a separate branch, codex/official-data-mt, and an isolated worktree
  from an appropriate committed baseline. Do not switch/reset/stash/clean the
  other AI's active working tree or silently copy its uncommitted edits.
- If that branch already exists, inspect/reuse it safely or choose an explicit
  unique suffix. Base on a tested committed revision; record which round-09
  changes are included/excluded. Do not claim unfinished round 09 is validated.
- Run the project gate and record baseline frozen-suite metrics and current
  source/catalogue/data identities. Repair baseline issues within scope or
  document pre-existing failures; never overwrite historical evidence.
- Inventory existing population, municipality bridges, sectors, aggregate
  execution and answer rendering. Reuse verified code rather than duplicating it.

Acceptance: independent workspace; recorded baseline; explicit source list and
stage ledger. Commit docs(docs): record official data integration baseline.

## Shared contract required for every source

Implement this foundation in stage 1 and apply it to all later stages.

Source registry:
- Stable dataset/resource IDs; official publisher; discovery, API/download and
  metadata URLs; actual licence/terms reference, retrieval date and attribution.
- States: discovered, verified, fetched, normalized, queryable, commercially
  enabled, stale, failed, or unavailable. State transitions require evidence.
  A dashboard or public search page is not automatically an open bulk API.
- Record reuse conditions and unresolved questions per resource. Do not invent
  a blanket CC BY licence. Keep ODbL-derived assets separable until the actual
  adapted-database obligations have been assessed and implemented.
- Expected publication cadence versus actual latest publication/competence,
  geographic/sector coverage, schema version, revision policy and limitations.
- Hashes, immutable raw/normalized paths, ingestion/build IDs, row totals,
  duplicate/quarantine/out-of-scope counts and recovery/checkpoint status.

Ingestion:
- Rust implementation using existing workspace dependencies where practical.
  No paid service, PostgreSQL, Redis or Elasticsearch. Use explicit HTTP timeouts,
  limited retries/backoff, rate limits, pagination and atomic publication.
- Fetch official metadata before selecting variables/periods or assuming an
  endpoint. Validate content/schema: HTTP 200 with HTML is not valid CSV data.
- Preserve source bytes and revisions outside Git. Every input row must be
  accounted for, including explicitly scope-filtered/rejected/quarantined rows,
  their selection predicates and reasons;
  no silent loss, trimming or replacement with zero.
- Handle encodings, separators, quoted fields, units, missing/suppressed cells,
  revisions and source-specific sentinel values explicitly.
- Completed immutable batches/checkpoints must survive failure; prove resume
  behavior with tests. Do not overwrite a serving artifact during refresh.
- Pin permitted public fields. Prefer aggregate business statistics; do not
  ingest unnecessary personal records or reconstruct anonymized identities.

Identity, time and storage:
- CNPJ is a string; root and establishment are distinct. School/CNES IDs are
  not CNPJ. Relationships may be one-to-many or absent.
- RFB/SIAFI and IBGE municipality codes are different. Version the bridge and
  territorial changes; expose unmatched/ambiguous joins. Never match only by name.
- Preserve observation/reference time, publication/availability time, ingestion
  time and revision separately. Support latest-available and known-as-of semantics.
  A later-published labour/production number must not leak into a historical query.
- Use sidecar assets with documented versioned schemas. Existing rusqlite is
  available if appropriate; indexed read-only SQLite or an established immutable
  representation is preferable to introducing another service.
- Keep existing .ocnpj/.ocagg semantics intact. If a container layout changes,
  update its format documentation/version and compatibility tests before use.
- Default data scope is MT for the pilot, but never present an MT-only artifact
  as national. Preserve national reference metadata necessary for valid joins.

Query and answers:
- Every source slice must have a typed request, validation, actual execution,
  pt-BR interpretation and an answer with source references and dates.
- Return unit, geographic grain, period, coverage, calculation and availability.
  Distinguish missing data, unavailable source, empty selection and observed zero.
- Add explicit source/capability-aware operations as needed; do not force new
  analytics into stock/count IR or concatenate answers as if a join existed.
- Example names such as IndicatorLookup, IndicatorTrend, IndicatorComparison,
  Ratio and Association are design proposals, not implemented operations.
- The current CLI has ask/eval-questions/catalog/agg/diff-exec. Data-management
  commands are proposals: implement, document and test them before invoking them.
  Do not invent flags in reports. Keep NLU interpretation separate from gold labels.
- A municipal statistic is never an observed fact about an individual CNPJ.
  Association is not causality, demand, profitability or a buyer-supplier link.

Common tests:
- Offline small fixtures with independently calculated expected values.
- Missing/suppressed/zero cells; duplicate keys; incompatible geography, unit or
  time; malformed input; changed schema; partial download; pagination overlap;
  retry limits; interrupted refresh/resume; unchanged artifact after failed refresh.
- Synthetic fixtures are clearly marked and separate from real-source validation.
- Meaningful integration tests must execute through normalization, query and
  response metadata. Live contract checks are separate from ordinary offline CI.

## Stage 1 — Source registry, asset lifecycle and typed capability foundation

Suggested implementation locations, subject to repository audit:
- crates/opencnpj-insights: source registry, normalized indicators and execution.
- crates/opencnpj-lab: discovery/import/verify/query development CLI.
- crates/opencnpj-query: typed operations and capabilities where needed.
- crates/opencnpj-nlu: actual pt-BR resolution for available operations.
- Existing renderer: source-aware answer envelopes.

Deliver source listing/status, verified import, immutable publication and
verification through real documented CLI commands. Include a source unavailable
response before adding domain data. Do not build a broad abstraction without
first proving the smallest complete source slice.

Acceptance: fixture import/query/refresh failure and resume tests pass; no
fallback to invented values; manifests distinguish licence/availability states.
Commits: feat(ingest): add verified source asset lifecycle;
feat(cli): expose source status and verification.

## Stage 2 — Essential IBGE context for MT

Order:
1. Verify/reuse municipality and population assets already implemented.
2. Add released Census demographic/household-income indicators at supported
   geographic levels. Do not promise unavailable bairro/sector variables.
3. Add municipal GDP and CEMPRE aggregates with their actual edition schemas.
   Select indicators after inspecting metadata; do not hardcode uniform
   sector columns across releases.

Deliver MT comparisons and dated indicators; protect existing density semantics
by filtering numerator and denominator to the same admitted geographic scope.
Distinguish GDP per capita, household income, employment remuneration and capital.
CEMPRE activity definitions are not interchangeable with RFB active status.

Business acceptance examples:
- Compare Sorriso and Sinop on a selected population reference.
- Show registered sector establishments per 10,000 inhabitants across an
  explicitly specified MT scope.
- Explain why municipal GDP per capita is not household purchasing power.
- Refuse/clarify an unavailable neighbourhood indicator rather than substitute
  the municipality silently.

Tests: known code joins, territorial changes, duplicate/missing municipalities,
numerator/denominator scope alignment, absent columns and denominator zero.
Real-data gate: MT fixture-independent checks against selected official table
cells/totals, with variable/table IDs, periods and extraction evidence.
Commits: feat(ingest): import MT socioeconomic indicators;
feat(cli): query source-backed municipal indicators.

## Stage 3 — Essential labour context: Novo Caged and RAIS

Import MT monthly Novo Caged aggregates and annual RAIS structural indicators,
with available municipal/sector dimensions and source revisions. Prefer official
published aggregates when sufficient; public non-identified microdata must not
be treated as identifiable company/worker data.

Deliver admissions, departures, balance and relevant structural indicators.
Do not derive a stock by summing flows without a valid baseline/revisions.
Do not label wage averages as household income or predict exact CNPJ headcount.

Acceptance examples:
- Which MT municipalities increased formal employment in a chosen sector over
  specified published months?
- Compare a sector's dated labour balance with its registered establishments,
  with both units and source periods visible.
- “How many employees does this CNPJ have today?” must disclose unavailable
  company-specific information instead of attaching a municipal total.

Tests: admission minus departure arithmetic; negative balances; revised months;
missing cells versus zero; double counting; sector aggregation and future-data
leakage. Compare MT totals to official releases for the same revision/coverage.
Commit feat(ingest) plus feat(cli) slices with tests and raw evidence.

## Stage 4 — MT agriculture: PAM, PPM, then LSPA

Import verified municipal annual crop/livestock indicators for MT. Add monthly
LSPA only for the geographic levels actually released; never synthesize monthly
municipal production by dividing an annual figure.

Deliver crop area/production and herd indicators; compare them with explicitly
defined related-CNAE establishment groups. Keep agriculture business vocabulary
and regional language separate from actual ownership/purchase facts.

Acceptance examples:
- Rank MT municipalities by a specified crop/area/year.
- Compare a declared cattle indicator with registered veterinary-service supply.
- Explain why a regional production total cannot identify a CNPJ's farm,
  machinery ownership or purchasing intent.

Tests: hectares versus other units, tonnes versus kilograms, animal categories,
missing/suppressed cells, annual versus monthly geography, zero denominators,
multi-CNAE deduplication and incompatible years.
Reconcile selected MT published cells/totals; do not compare unlike concepts.
Commit a working annual slice before adding monthly coverage.

## Stage 5 — MT fleet, then ANTT transport specialization

First import Senatran municipal fleet by available vehicle type/age dimensions.
Then inspect RNTRC resources separately: aggregate history and identified
resources have different schemas, coverage and permitted uses.

Deliver territory-level fleet/related-business ratios and transport indicators.
Do not allocate a municipality's fleet to a CNPJ or invent vehicle ownership.

Acceptance examples:
- Compare registered truck totals across selected MT municipalities/months.
- Rank defined territories by vehicles per related establishment, with explicit
  classifications, missing values and period alignment.
- Clarify or refuse a company-fleet ownership request not supported by the
  admitted resource.

Tests: registered versus circulating fleet terminology; type changes; duplicated
rows; unknown municipalities; many-to-many CNAE joins; zero business count;
geographic aggregates misused as individual records.
Validate selected official fleet totals and RNTRC schema/coverage independently.

## Stage 6 — First usable MT release and analytical joins

This is the first-release milestone, not permission to stop the entire order.
Expose working source-backed MT questions across stages 2–5 through the CLI.
Deliver explicit coverage/status reporting and useful unavailable-data responses.

Implement only well-defined cross-source ratios/comparisons initially.
For correlation requests, require paired observations, aligned coverage/period,
sample N, missing/outlier policy and a documented method. If implemented,
test positive/negative/undefined coefficients and minimum-sample handling;
show associations as descriptive. Otherwise keep the capability unavailable,
not an invented coefficient.

Required evidence:
- At least 120 distinct meaningful integration/semantic acceptance cases for
  this first release, including positive, clarification, refusal, unavailable,
  missing-data and temporal cases. No filler-only expansion to reach the quota.
- Complete machine-readable outcomes and independent arithmetic/reference
  checks; an operation whose backend is single-path is labelled accordingly.
- Run existing frozen interpretation suites and targeted prior-round replays.
  Keep old labels and semantic adjudication overlays separately identified.
- Counts of unsupported versus incorrectly interpreted requests are separate.
- No statement of human product validation or target-hardware performance
  without the corresponding evidence.

Tag the accepted commit with a descriptive MT data milestone only if the
repository's release practice supports it; a local milestone report is sufficient.
Do not create a deployment or public release automatically.

## Stage 7 — Government-sales pack: PNCP, Compras and CGU

After the MT core, integrate plans/notices, awards/contracts and suppliers from
actual PNCP/Compras endpoints. Add CGU CEIS/CNEP sanctions with effective dates.
Default daily collection for procurement is an implementation policy, not a
claim of guaranteed real-time official coverage.

Deliver MT public-opportunity search, supplier context and expiring-contract
queries. Preserve procurement IDs and cross-portal deduplication.
Estimated, awarded, contracted and paid amounts are distinct; only show paid
amounts if an admitted payment resource actually supplies them.
Do not generalize public procurement to private B2B transactions.

Tests: pagination/retries; duplicate portals; contract amendments/cancellations;
effective-date boundaries; CNPJ root versus establishment; decimal amounts and
currency; repeated items versus contract totals; sanction scope and expiration.
No sanction record is not a universal certificate of good standing.

## Stage 8 — Additional verticals and local enrichment

Implement in this sequence, each as its own vertical slice:
1. CNES: establishments/services/capacity, with CNES IDs and available CNPJ
   relationships; exclude unnecessary personal-professional fields.
2. INEP Censo Escolar: dated school/infrastructure aggregates; school IDs are
   not CNPJ. Verify available linkage fields rather than infer identities.
3. ANP: sampled weekly fuel observations and coverage. No claim of all stations,
   all municipalities or live prices.
4. Comex Stat: monthly territory/product trade statistics. Municipal detail
   reflects fiscal domicile and SH4 coverage; do not manufacture exporting-CNPJ
   lists, customer identities or municipal NCM detail not released.
5. BCB: explicitly selected dated economic series, documented units/frequencies
   and revisions; no company banking balances or credit limits.
6. MT data/geography portals: admit only individually verified datasets that
   fill a recorded gap. A dashboard/category alone is not an integration.

For each: registry evidence, actual adapter/store/query/NLU slice, source-specific
tests, fixture plus real-data reference checks and its own measured report.
Add new acceptance cases for each vertical; do not count the same case repeatedly
as evidence for different unexecuted integrations.

## Stage 9 — Licence-sensitive energy and fiscal context

ANEEL: keep the ODbL resource and derivations appropriately separated. Record
the actual licence obligations and implement required attribution/share-alike
delivery behavior before commercial enablement. Do not automatically mark the
whole proprietary application as open or assume that combining databases has
no consequences. Unknown reuse conditions leave commercial enablement pending,
not silently approved.

PGFN: preserve quarterly reference, debt state and federal/FGTS scope. Do not
present a record as insolvency, a universal credit score, every tax liability
or a current tax-clearance certificate. Keep personal debt records out of the
business prospecting feature.

Tests: licence-state admission; source metadata on derivatives; effective/revision
dates; duplicate debtor/inscription relationships; debt-state distinctions;
aggregate-to-individual misuse. Source unavailability must not become “no debt”.

## Stage 10 — Product/API completion and full delivery

Expose the admitted capabilities through the existing application/API
architecture after inspecting it. Do not introduce an unauthenticated arbitrary
query/SQL endpoint or claim the local CLI state file is signed HTTP context.
Define actual request/response contracts, pagination, capability errors, source
status and provenance; update docs/API.md and relevant design documents.

API tests must cover parameter validation, unavailable sources, source/date
metadata, context isolation, incompatible versions, pagination and typed-query
execution. Offline tests use realistic small fixtures; live-source acceptance
runs retain actual commands and output outside Git.

Run the full gate and regression set; verify all stages' real-data checks,
manifest identities and row accounting. Publish a final implementation report:
what is integrated/queryable/commercially enabled, what is stale/unavailable,
licence decisions, data coverage, test counts, raw evidence locations/hashes,
commit list, remote branch and remaining external dependencies.

The task is complete only when the above acceptance criteria are met for all
available/admitted components. A documented external blocker is not an
implemented feature; state it plainly and finish independent work.

## Required validation and commit cadence

For every implementation milestone:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

Use the existing Cargo cache/target configuration where appropriate; record exact
commands and exit codes. Never claim skipped national/live tests passed. Ordinary
CI must not depend on a live public endpoint. For changes affecting snapshot
layout/source round-trip paths, apply the repository's format/version and
opencnpj verify --sources requirements as appropriate.

Commit self-contained code and required fixtures together, after relevant tests
and the gate. Commit each source/feature slice rather than one giant final diff.
Examples use allowed scopes and subjects <=72 characters:
- docs(docs): define official source contracts and MT milestones
- feat(ingest): add verified MT labour aggregates
- feat(cli): query MT agriculture and fleet indicators
- test(cli): cover cross-source time and identity boundaries
- docs(docs): record measured MT data integration results

Push each completed milestone to the isolated codex/ branch. Do not stage all
files blindly. Check staged filenames/sizes and hashes before commit/push.
No national datasets, large outputs, credentials or unrelated round-09 files
belong in these commits. Commit manifests, small permitted fixtures, source
registries/licence records, tests, compact summaries and reproducible authoring/
ingestion tools. Preserve byte hashes of frozen fixtures.

If GitHub authentication/network access fails, preserve local commits, report
the exact failure and continue local independent work. Never force-push, change
credentials or claim an unsuccessful push succeeded.

## Required outputs

Use a dedicated docs/intelligence/results/official-data-mt/ tree:
- BASELINE.md, stage-ledger.json and source registry/licence references.
- Per-stage COMMANDS.md, compact summary, tests/gate logs, source/data hashes,
  fixture descriptions, real-reference checks and unresolved dependencies.
- A capability coverage list linking every implemented source/operation to
  executable acceptance cases and pt-BR examples.
- FINAL-REPORT.md with completion boundaries and commit/push evidence.

Raw immutable data and large execution outputs remain outside Git in documented
paths. Keep raw-output hashes and reproduction commands. Do not call hashes
downloadable archives or pretend local temporary files are portable artifacts.
