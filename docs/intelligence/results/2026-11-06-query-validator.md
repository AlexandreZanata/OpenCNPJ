# 2026-11-06 — WP-2.2 validator [MEASURED]

Base commit `7a15ce9e1eb95e28437d54b03cc067850e14d0c1`. Compiler
`rustc 1.99.0 (b940084d7 2026-09-28)`.

## Delivery

`opencnpj-query::validate` exposes
`validate(Request, &Capabilities) -> Result<ValidRequest, Vec<Issue>>`.
`ValidRequest` wraps a private tuple field; the validator is its only
constructor, pinned by a compile-fail doc test. Each `Issue` has an
`IssueKind` (`Missing`, `Ambiguous`, `Impossible`), a stable `IssueCode`
and complete `ValidRequest` options for the clarification. Options are
executable as-is.

`Capabilities` describes the served competence, the stock competences kept
for history, the day range covered by flow tables, the available population
references, the catalogue versions, the curated sector count, the geography
assets and the caller list limit. Callers canonicalize first (WP-2.1) and
validate the canonical request.

## Rules and tests

| Rule | Code | Kind | Test |
|---|---|---|---|
| Catalogue version mismatch | `CatalogueVersion` | Impossible | `catalogue_versions_must_match` |
| Empty place, activity or porte filter | `EmptyFilter` | Impossible | `empty_filters_are_rejected` |
| Empty status set | `EmptyStatus` | Impossible; option restores the operation default | `closures_require_closed_status` |
| Population metric without a place dimension or filter | `PopulationNeedsGeography` | Missing | `population_metric_needs_place_dimension` |
| Population reference not available | `PopulationUnavailable` | Impossible; option uses an available table | `population_reference_must_be_available`, `population_filter_needs_reference` |
| Population filter without a reference | `MissingPopulationRef` | Missing; option picks an available table | `population_filter_needs_reference` |
| Active change without two available snapshots | `ActiveChangeUnavailable` | Impossible; option falls back to `Flow(Registrations)` (SR-05) | `active_change_requires_snapshots` |
| Active change without a period | `MissingPeriod` | Missing; option uses the last two competences | `active_change_missing_period_offers_default` |
| Percentage change below the minimum base | `ActiveChangeMinBase` | Missing; option sets `min_base = 30` (SR-05) | `percent_change_needs_minimum_base` |
| Stock at an unavailable date | `SnapshotUnavailable` | Impossible; option uses the current or latest competence | `stock_at_past_date_needs_snapshot` |
| Flow or trend without a period | `MissingPeriod` | Missing; option uses the last 12 months | `trend_needs_period_and_two_grains` |
| Trend shorter than two grains | `TrendNeedsTwoGrains` | Impossible; option uses the last 12 months | `trend_needs_period_and_two_grains` |
| Period kind does not match the metric | `WrongPeriodKind` | Impossible; option clears or converts the period | `period_kind_must_match_metric` |
| Flow period outside the flow tables | `PeriodOutsideData` | Impossible; option clips when the intersection is non-empty | `flow_period_must_be_covered` |
| Closure flow without status `08` | `ClosedFlowStatus` | Impossible; option pins status closed (SR-04) | `closures_require_closed_status` |
| Comparison member count or dimension | `CompareMembers` | Missing for 0–1, Impossible above 4 or mixed dimensions; option truncates to four | `compare_requires_members_of_the_dimension` |
| Sector ranking or comparison with fewer than two sectors | `SectorCatalogueTooSmall` | Impossible | `sector_ranking_needs_the_catalogue` |
| MEI with a porte selection excluding micro | `MeiPorteContradiction` | Impossible; option removes the porte filter (SR-08) | `mei_contradicts_small_and_other_porte` |
| Contact filter outside a list | `ContactOnlyForList` | Impossible; option clears the filter | `list_limits_and_contact` |
| List limit above the caller plan | `ListLimit` | Impossible; option clamps to the plan limit | `list_limits_and_contact` |
| Page invalid for the operation | `InvalidPage` | Impossible; option resets to page one | `list_limits_and_contact` |
| Empty day, capital or population range | `InvalidRange` | Impossible; option clears the range | `invalid_ranges_are_rejected` |
| Near filter without geography assets | `NearNeedsGeography` | Impossible; option uses the centre municipality | `near_filter_needs_geography` |
| Ranking limit zero | `InvalidRankLimit` | Impossible; option uses `k = 10` | `rank_limit_must_be_positive` |
| Head-office unit with a branches-only filter | `HeadOfficeUnit` | Impossible; option clears the filter | `head_office_unit_contradicts_branches_only` |
| Capital filter with establishments | — | Allowed; renderer adds the company-level caveat | `capital_filter_is_allowed_with_establishments` |
| Lookup CNPJ check digits | — | Enforced by `CnpjRef` at construction and deserialization | `lookup_ids_are_type_checked` |

Place homonyms and ambiguous sector terms are resolved before the IR:
the request carries concrete catalogue ids and cannot express an
unresolved term. `IssueKind::Ambiguous` is reserved for those interpreter
decisions; this validator produces `Missing` and `Impossible` for the
executed IR.

## Acceptance

* One valid and one invalid case per rule: 25 contract tests.
* `ValidRequest` cannot be constructed outside the validator: compile-fail
  doc test.
* Issues carry clarification options as complete IR values:
  `issues_serialize_options_as_requests` and the option assertions in the
  rule tests.
* All 36 canonical golden requests from WP-2.1 validate:
  `golden_requests_validate`.

## Commands

```bash
export CARGO_TARGET_DIR=/data/opencnpj-target
git rev-parse HEAD
rustc -V
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p opencnpj-query --test validate_contract
cargo test -p opencnpj-query --doc
git diff --check
```

Raw output is in
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-2.2-validator.log`.
Selected lines follow.

```text
7a15ce9e1eb95e28437d54b03cc067850e14d0c1
rustc 1.99.0 (b940084d7 2026-09-28)
running 25 tests
test active_change_missing_period_offers_default ... ok
test active_change_requires_snapshots ... ok
test capital_filter_is_allowed_with_establishments ... ok
test catalogue_versions_must_match ... ok
test closures_require_closed_status ... ok
test compare_requires_members_of_the_dimension ... ok
test empty_filters_are_rejected ... ok
test flow_period_must_be_covered ... ok
test golden_requests_validate ... ok
test head_office_unit_contradicts_branches_only ... ok
test invalid_ranges_are_rejected ... ok
test issues_serialize_options_as_requests ... ok
test list_limits_and_contact ... ok
test lookup_ids_are_type_checked ... ok
test mei_contradicts_small_and_other_porte ... ok
test near_filter_needs_geography ... ok
test percent_change_needs_minimum_base ... ok
test period_kind_must_match_metric ... ok
test population_filter_needs_reference ... ok
test population_metric_needs_place_dimension ... ok
test population_reference_must_be_available ... ok
test rank_limit_must_be_positive ... ok
test sector_ranking_needs_the_catalogue ... ok
test stock_at_past_date_needs_snapshot ... ok
test trend_needs_period_and_two_grains ... ok
test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
test crates/opencnpj-query/src/validate.rs - validate::ValidRequest (line 145) - compile fail ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

The full workspace test suite and the delivery gate passed with the same
log. The validator adds no dependency to `opencnpj-query`.
