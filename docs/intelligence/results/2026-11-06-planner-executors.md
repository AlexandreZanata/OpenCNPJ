# 2026-11-06 — WP-2.3 planner and executors [MEASURED]

Base commit `5cd7be876684ed97b67956a8b2e5b24684daaa36`. Compiler
`rustc 1.99.0 (b940084d7 2026-09-28)`. The latency run used
`taskset -c 0` as the one-vCPU emulation on the build host.

## Delivery

* `opencnpj-insights::plan` selects `Aggregates`, `Engine` or `Static` with
  a static capability table and a reason.
* `opencnpj-insights::execute` reads the monthly `.ocagg` artifacts and
  executes counts, comparisons, rankings, breakdowns and trends. It covers
  stock, flows, active change, density, inhabitants-per and share, with
  place, activity, status, porte, MEI, Simples, head-office, exclusion and
  population filters.
* `opencnpj-lab::query` lowers the canonical IR to the `opencnpj::params`
  grammar, executes the same IR in process through
  `opencnpj_store::search`, and provides the HTTP engine client with a
  service key, timeout and bounded concurrency.
* `opencnpj-intel diff-exec` compares both executors on the golden IR set;
  `bench-exec` records per-case latency; `remote-count` counts through a
  running API.

## Capability table

| IR shape | Backend | Reason |
|---|---|---|
| `Meta` | Static | Catalogue or methodology answer |
| `List`, `Lookup` | Engine | Snapshot documents, plan limits and contacts |
| Stock count/compare/rank/breakdown with capital, natureza, opened, status-date, contact or secondary CNAE | Engine | Filters outside the aggregate tables |
| Stock with status other than active plus porte/MEI/Simples | Engine | T2 carries only status and head office |
| Stock with current/active dimensions | Aggregates | T1/T2 cells and marginals |
| Registrations, closures or net registrations with an aligned month period | Aggregates | T3/T4 buckets |
| Flow with an unaligned period or MEI opt-in/exit | Engine | Exact days or columns absent from the aggregates |
| Active change | Aggregates | Two historical T1/T2 snapshots |
| Trend | Aggregates when aligned, otherwise Engine | Flow buckets |
| Year/month dimension on non-trend stock | Engine | No stock date dimension in the aggregates |

## National differential

```bash
agg=/data/opencnpj-data/intelligence/agg/2026-10.ocagg
history=/data/opencnpj-data/intelligence/backfill/aggregates
snapshot=/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj
catalogue=/data/opencnpj-data/intelligence/catalog-municipalities/2026-10/municipalities.tsv
taskset -c 0 /data/opencnpj-target/release/opencnpj-intel diff-exec \
  --agg "$agg" --history "$history" --snapshot "$snapshot" \
  --catalog "$catalogue" --sectors crates/opencnpj-insights/catalog/sectors.toml \
  --cases crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl
```

Raw output is in
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-2.3-diff.log`.
The full golden set has 36 cases; the summary is:

```text
summary	compared=22	mismatches=0	aggregate_only=3	engine_only=6	engine_n/a=4	invalid=0
```

The 22 aggregate/engine comparisons include every current stock shape, the
aligned 2025 registration flow, density, inhabitants-per, exclusions,
near-place expansion, sector and CNAE rankings and the sector share
breakdown. Selected matches:

```text
match	padarias_sorriso
match	compare_sorriso_com_sinop
match	empresas_abertas_2025_mt
match	compare_density_sorriso_sinop
match	breakdown_porte
match	exclusions
match	rank_inhabitants_minimum
match	near_sinop_km
match	rank_cnae_section
```

The three aggregate-only cases are historical stock or active change; the
engine keeps only the current snapshot by design. The six engine-only cases
are unaligned trends, MEI flows and filters outside the aggregate tables.
The four `engine_n/a` cases are `List` and `Lookup`, which the in-process
executor does not serve; their lowering is covered by the unit test.

## Fixture, lowering and HTTP tests

`cargo test -p opencnpj-lab --bin opencnpj-intel query::tests` builds the
fixture snapshot, aggregate, municipality catalogue and sector map and
checks:

* seven representative requests (count, compare, breakdown, rank, closed
  stock, density and head office) produce equal aggregate and engine values;
* the count lowering parses into the expected situations, municipality,
  CNAE and head-office filters;
* the list lowering carries the limit, offset, capital range, contact
  filter and sort;
* the HTTP client sends the bearer service key, the limit and the
  establishment filter, parses `total`, and preserves order with bounded
  concurrency against an axum mock.

Raw output is in
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-2.3-gate.log`.

## Latency on one vCPU

```bash
taskset -c 0 /data/opencnpj-target/release/opencnpj-intel bench-exec \
  --agg "$agg" --history "$history" --snapshot "$snapshot" \
  --catalog "$catalogue" --sectors crates/opencnpj-insights/catalog/sectors.toml \
  --cases crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl
```

Raw output is in
`/data/opencnpj-data/intelligence/results/2026-11-06-wp-2.3-bench.log`.
Selected per-case times in microseconds:

```text
case	                        backend	    aggregate_us	engine_us
padarias_sorriso	            Aggregates	        215	    34022
sorriso	                    Aggregates	      58704	     3363
empresas_abertas_2025_mt	    Aggregates	     255410	    67623
rank_inhabitants_minimum	    Aggregates	        195	   315156
rank_cnae_section	            Aggregates	      79061	  1460124
total		                               1620774	  6206566
```

The aggregate path is a full scan of the relevant `.ocagg` cells in this
first implementation; the engine path uses postings and doc values. The
one-run numbers include first-touch page faults and are not a throughput
claim. The aggregate wins on the density and ranking cases; the engine wins
on small filtered counts. The next step for the aggregate path is the
marginal/index fast path already present in the reader.

## Limits and follow-ups

* Active change and historical stock remain aggregate-only because the
  backfill intentionally removed the past snapshots.
* The in-process engine does not execute `List`, `Lookup` or `Meta`; the
  HTTP client exposes counts through `/api/v1/search`.
* The execution `Value` model (scalar, ratio and keyed rows) is the input to
  the WP-2.4 renderer; labels, provenance and templates are not part of this
  work package.
* Sector IDs are dense file-order IDs supplied by the caller; the
  interpreter owns their final assignment.
* The aggregate executor supports the capital filter only through the
  engine, as decided by the capability table.
