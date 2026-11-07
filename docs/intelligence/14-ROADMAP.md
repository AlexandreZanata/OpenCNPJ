# 14 — Roadmap: staged work packages for the executing agent

This is the execution plan. Each work package (WP) is self-contained: goal, inputs,
deliverables, acceptance criteria and verification. An agent can pick any WP whose
dependencies are done. The research order of the brief (§14: audit → research → language
design → data → isolated PoCs → evidence-based decision → integration) is preserved:
**phase 6 starts only after the owner approves the decision record of phase 5.**

## 0. Rules for whoever executes this plan

1. Read, in order: [README](README.md), [01-REQUIREMENTS](01-REQUIREMENTS.md),
   [02-AUDIT](02-AUDIT.md), [04-QUERY-IR](04-QUERY-IR.md), then the documents named by the WP.
   Repository rules in `AGENTS.md` apply to every WP (English only; correctness first;
   no performance number without command and raw output; no PostgreSQL/Redis/Elasticsearch
   or paid services; no national datasets or large binaries in Git; commit message format;
   **commit or push only when the owner asks**).
2. Phases 0–5 **do not modify existing crates** (`opencnpj-core`, `-store`, `-ingest`,
   `opencnpj`, `-desktop`), the `.ocnpj` format or the public API. The only allowed edit
   outside new files is adding workspace members in the root `Cargo.toml` (WP-0.1) and
   new workspace dependencies needed by new crates.
3. Build host specifics (02-AUDIT.md §8, 01-REQUIREMENTS.md §8): `/` is full. Always
   `export CARGO_TARGET_DIR=/data/opencnpj-target`; put data under
   `/data/opencnpj-data/intelligence/` (`datasets/`, `eval/`, `packs/`, `agg/`, `catalog-raw/`,
   `results/`). The national snapshot is `/data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj`;
   never modify or delete anything under `/data/opencnpj-data/snapshots` or `raw/`.
4. Every WP ends with the delivery gate:
   ```bash
   cargo fmt --all --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   ```
5. Results: any measured number goes into `docs/intelligence/results/<date>-<topic>.md`
   using the template of 12-BENCHMARK-PROTOCOL.md §8; raw outputs under
   `/data/opencnpj-data/intelligence/results/`. Label everything MEASURED / ESTIMATE /
   HYPOTHESIS.
6. If a gate or acceptance criterion fails, record the result honestly, do not weaken the
   test, and stop that WP with a short report to the owner.
7. New dependencies need a one-line justification in the WP report (crate, version,
   licence, why the workspace's existing crates do not suffice). Pre-approved if needed:
   `regex-automata` 0.4 (constructions automaton), `hmac` (context token), `toml` (data
   files). Anything else: ask.
8. The executing agent **must not write the H-blind evaluation set** (10-SYNTHETIC-DATA.md §6).

## 1. Crate layout (created in WP-0.1)

| Crate | Kind | Depends on | Contents |
|---|---|---|---|
| `crates/opencnpj-query` | lib | `opencnpj-core`, `serde` | IR types, canonicalization, codec, cache key, validator, textual printer |
| `crates/opencnpj-nlu` | lib | `opencnpj-query`, `opencnpj-core`, `fst` | Normalizer, language pack (compiler + loader), spotter, time grammar, constructions, frame resolver, dialogue acts and context, policy, optional classifier inference |
| `crates/opencnpj-insights` | lib | `opencnpj-query`, `opencnpj-store` | Catalogues (load), `.ocagg` writer/reader, planner, aggregate executor, engine lowering + in-process/HTTP engine executors, renderer and templates |
| `crates/opencnpj-lab` | bin `opencnpj-intel`, `publish = false` | all of the above + the `opencnpj` library (for `params::parse` and the engine in tests) | Catalogue builders, aggregates builder CLI, synthetic generator, training, evaluation, benchmarks, diff tests |

Rationale (vs. a separate `opencnpj-intelligence/` tree): one workspace keeps one
lockfile, one delivery gate and the shared normalization/CNPJ code; the language crates
stay free of storage and HTTP dependencies, so they remain reusable by other projects.
Research code is isolated in `opencnpj-lab`, which the product never links.

## 2. Phases, milestones and dependencies

```text
Phase 0  Setup & ground truth      WP-0.1 ─┬─ WP-0.2 ── WP-0.3 (owner collects H-blind in background)
Phase 1  Data assets                        ├─ WP-1.1 ─ WP-1.2 ─┐
                                            │           WP-1.3 ─┤
                                            │  WP-1.4 (needs 2.1 types, 1.2) ── WP-1.5 (background, ops)
Phase 2  IR & execution                     ├─ WP-2.1 ─ WP-2.2 ─ WP-2.3 (needs 1.4) ─ WP-2.4
Phase 3  Evaluation infrastructure          │  WP-3.1 (needs 1.2, 1.3, 2.1) ─ WP-3.2 ─ WP-3.3
Phase 4  Proofs of concept                  │  WP-4.1 (A) ─┬─ WP-4.3 (D) ─ WP-4.4 (curves)
                                            │  WP-4.2 (B) ─┘   WP-4.5 (C, only via gate G-C)
                                            │  WP-4.6 dialogue/context (needs 4.1)
Phase 5  Decision                           └─ WP-5.1 ── OWNER APPROVAL ──┐
Phase 6  Product integration                   WP-6.1 … WP-6.6           ◄┘
Phase 7  Continuous improvement                WP-7.1, WP-7.2
```

| Milestone | Done when |
|---|---|
| M1 Data ready | WP-1.2, 1.3, 1.4 accepted (catalogues + current aggregates, diff test green) |
| M2 Queries without language | WP-2.1–2.4 accepted: an IR written by hand returns a correct, rendered answer |
| M3 Measuring stick | WP-3.1–3.3 accepted; H-dev, ADV, OOD frozen |
| M4 Evidence | WP-4.1–4.4, 4.6 (and 4.5 if triggered) reported |
| M5 Decision | WP-5.1 decision record approved by the owner |
| M6 v1 beta | WP-6.1–6.4 accepted |
| M7 v1 | WP-6.5–6.6 accepted; definition of done of 01-REQUIREMENTS.md §9 met |

Parallel tracks after WP-0.1: {WP-1.1→1.2, WP-1.3}, {WP-2.1→2.2}, {WP-0.2}. WP-1.5 runs
as a background job once WP-1.4's builder exists.

---

## Phase 0 — Setup and ground truth

### WP-0.1 Workspace skeleton — size S
* **Goal:** create the four crates of §1 with module stubs and crate docs; wire them into
  the workspace.
* **Deliverables:** `crates/opencnpj-{query,nlu,insights,lab}/` (Cargo.toml using
  `workspace = true` fields, `src/lib.rs` or `src/main.rs`, README.md per crate); root
  `Cargo.toml` members updated.
* **Acceptance:** delivery gate passes; `git diff --stat` shows no change in existing crates;
  `cargo build -p opencnpj-lab` produces `opencnpj-intel` printing its subcommand list.

### WP-0.2 Annotation guide and H-dev — size M
* **Goal:** make "gold IR" unambiguous and produce the development set.
* **Inputs:** 01-REQUIREMENTS.md §5 (SR rules), 04-QUERY-IR.md, 07 §4.
* **Deliverables:** `docs/intelligence/ANNOTATION-GUIDE.md` (how to label decision
  execute/clarify/reject, defaults, ambiguity, units, time anchoring, with ≥ 40 worked
  examples); `crates/opencnpj-lab/eval/h-dev.jsonl` (≥ 300 items covering every intent,
  metric, act and SR rule; AI-drafted items flagged `"origin":"ai-draft"` until a human
  reviews them).
* **Acceptance:** every item validates against the IR schema (test); per-intent counts
  ≥ 15; owner review of a 50-item sample recorded.

### WP-0.3 Blind-set collection kit — size S (owner runs the collection)
* **Goal:** let the owner collect ≥ 500 questions and ≥ 50 short dialogues from people who
  never saw templates or constructions.
* **Deliverables:** `docs/intelligence/BLIND-SET-PROTOCOL.md` (instructions for
  participants with scenario prompts per persona but **no example phrasings**, consent and
  anonymization notes, CSV template, double-annotation procedure, freeze and hashing
  procedure). Storage: `/data/opencnpj-data/intelligence/eval/h-blind/` (outside Git until
  the decision is made); only its SHA-256 is committed.
* **Acceptance:** owner confirms the protocol; the evaluation harness (WP-3.2) refuses to
  run H-blind unless invoked with `--milestone`.

## Phase 1 — Data assets

### WP-1.1 RFB reference extraction — size S
* **Goal:** export the snapshot's reference tables and the set of município/CNAE codes
  actually used, as inputs for the catalogues.
* **Deliverables:** `opencnpj-intel catalog rfb --snapshot S --out DIR` writing
  `rfb_municipios.tsv`, `rfb_cnaes.tsv`, `rfb_naturezas.tsv`, `used_codes.tsv` (code, field,
  document count, UF distribution).
* **Acceptance:** counts equal the reference tables (5,572 municípios, 1,359 CNAEs for
  2026-10); `used_codes` totals equal `/api/v1/stats` totals on the same snapshot (test on
  the fixture + one run on the national snapshot, recorded).

### WP-1.2 Municipality and geography catalogue — size M
* **Goal:** the catalogue of 09-DATA-ASSETS.md §2.
* **Inputs:** Tesouro `tabmun.csv`; IBGE Localidades API; SIDRA 4709/93 (2022) and
  6579/9324 (latest estimate); IBGE municípios limítrofes 2024; Localidades do Brasil 2022
  GeoPackage (read with `rusqlite`). Download into `/data/opencnpj-data/intelligence/catalog-raw/<date>/`
  with a manifest (URL, date, size, SHA-256); record each source's terms of use.
* **Deliverables:** `opencnpj-intel catalog municipalities …` → `crates/opencnpj-insights/catalog/municipalities.tsv`
  + `overrides.tsv` (reviewed: `1182 → 5101837`, `9707` EXTERIOR) + `manifest.json`;
  loader in `opencnpj-insights`.
* **Acceptance (tests):** every TOM code used in the snapshot is mapped or overridden; every
  mapped municipality has UF, accented name, census and estimate population, coordinates;
  neighbour relation is symmetric; spot checks: 7107 → 3550308 São Paulo/SP, 9907 →
  5107925 Sorriso/MT, 8985 → 5107909 Sinop/MT; report of RFB vs IBGE name differences
  committed for review; rebuild from the same raw files is byte-identical.

### WP-1.3 CNAE hierarchy and sector catalogue v1 — size L
* **Goal:** 09-DATA-ASSETS.md §3.
* **Inputs:** IBGE CNAE API v2 subclasses (hierarchy + 17,180 activity phrases); CONCLA
  correspondence tables for legacy codes; RFB CNAE table.
* **Deliverables:** `catalog/cnae.tsv` (code, level, parent, official name, legacy mapping);
  `catalog/sectors.toml` (≥ 200 curated sectors with core/extended sets, aliases with
  singular/plural/gender); generated `catalog/sector_aliases_auto.tsv`; ambiguity and
  reachability reports.
* **Acceptance:** every RFB code has a CNAE 2.3 entry or a documented legacy mapping;
  every curated set contains only valid codes; no alias maps to two sectors without being
  listed in the ambiguity report; owner review of the top 50 sectors recorded (these drive
  most answers).

### WP-1.4 Aggregates `.ocagg` v0.1 — size L
* **Goal:** 09-DATA-ASSETS.md §4: spec, writer, reader, builder, verification.
* **Depends on:** WP-1.2 (dense municipality ids), WP-2.1 (id types).
* **Deliverables:** `docs/intelligence/AGGREGATES-FORMAT.md`; `opencnpj-insights::agg`
  (writer, mmap reader with bounds and checksum validation, cell iteration, marginals);
  `opencnpj-intel agg build --snapshot S --catalog C --out F.ocagg`;
  `opencnpj-intel diff-agg` (12-BENCHMARK-PROTOCOL.md §1).
* **Acceptance:** fixture test: aggregates equal brute-force counts from decoded records;
  national run: marginals for every CNAE and every UF equal the engine's exact counts,
  10,000 seeded random cells equal engine counts (0 mismatches); file size, build time and
  peak RSS recorded (MEASURED) and compared with the ESTIMATEs of 13; corrupted/truncated
  files are rejected (tests, like the snapshot's).

### WP-1.5 Historical backfill — size M (mostly wall-clock)
* **Goal:** T1/T2 aggregates for the competences chosen in D-04 (default 24 months).
* **Steps:** check `opencnpj download --month 2025-04 --dry-run`; if the default share does
  not list past months, use the existing `RFB_BASE_URL` / `RFB_SHARE_TOKEN` overrides for the
  share that lists 2023-05 … 2026-10 (09-DATA-ASSETS.md §1) — no code change. Per month:
  download → build → verify → `agg build --tables T1,T2` → delete ZIPs and snapshot. One
  month at a time, `nice -n 19 ionice -c3`, logs under `intelligence/backfill/`.
* **Acceptance:** one `.ocagg` per competence with manifest (source snapshot SHA-256,
  verify result); month-over-month national active totals plotted in the result file for a
  sanity check (no unexplained jumps); disk peak recorded.

## Phase 2 — Query IR and execution

### WP-2.1 IR core — size M
* **Goal:** 04-QUERY-IR.md §2–§3, §6, §9 in `opencnpj-query`.
* **Deliverables:** types; constructors that enforce canonical sets; `canonicalize()`;
  binary codec with `IR_VERSION`; `cache_key()`; serde JSON; textual printer.
* **Acceptance:** property tests (proptest): `decode(encode(x)) == x`; canonicalize is
  idempotent; permutations of set members give equal bytes; golden files for ≥ 30 IR
  examples (including all of 04 §1 and §5); encoding sizes reported (MEASURED).

### WP-2.2 Validator — size M
* **Goal:** 04-QUERY-IR.md §4: `validate(Request, &Capabilities) -> Result<ValidRequest, Vec<Issue>>`.
* **Acceptance:** one test per rule (valid and invalid case); `ValidRequest` cannot be
  constructed outside the validator (compile-fail doc test); issues carry what a
  clarification needs (options as IRs).

### WP-2.3 Planner and executors — size L
* **Goal:** 04-QUERY-IR.md §7: capability table, aggregate executor, engine lowering to
  `params::parse` pairs, in-process engine executor (`opencnpj_store::search`), HTTP engine
  client (reqwest, service key, timeouts, bounded concurrency).
* **Depends on:** WP-1.4, WP-2.2.
* **Acceptance:** lowering tests (in `opencnpj-lab`, which may depend on the `opencnpj`
  library; `opencnpj-insights` itself must not, to avoid a dependency cycle once phase 6
  makes `opencnpj` depend on it): every lowered IR parses with the existing
  `opencnpj::params::parse` into the expected `Query`; differential test on
  the fixture and on the national snapshot: aggregate executor == engine executor for the
  golden IR set (0 mismatches); latency of each executor on the golden set recorded on the
  1-vCPU emulation (MEASURED).

### WP-2.4 Renderer — size M
* **Goal:** 08-RESPONSES.md: envelope, blocks, templates, formatters, caveats, suggestions.
* **Acceptance:** template lints (§2) as tests; snapshot tests of rendered answers for the
  golden IR set; pt-BR formatting tests (thousands, decimals, percent, dates, plural and
  gender agreement); no block contains a number that is not in the result/provenance
  (test walks the output).

## Phase 3 — Evaluation infrastructure

### WP-3.1 Synthetic generator — size L
* **Goal:** 10-SYNTHETIC-DATA.md §1–§5, §8–§9.
* **Depends on:** WP-1.2, WP-1.3, WP-2.1.
* **Deliverables:** `opencnpj-lab::synth`, family specs `crates/opencnpj-lab/synth/pt-BR/`
  (≥ 60 families, ≥ 8 templates each, two authors), noise model, splits, manifest,
  diversity metrics, dialogue generator.
* **Acceptance:** determinism test (same id → byte-identical files); split leakage tests
  (no family, held-out slot value or reserved noise type crosses splits); every gold IR
  validates; manifest diversity metrics present; a 200-item random sample reviewed by a
  human for naturalness (record the share judged unnatural; target < 5 %).

### WP-3.2 Evaluation and benchmark harness — size M
* **Goal:** 12-BENCHMARK-PROTOCOL.md §1–§6 (`eval`, `bench-nlu`, `bench-agg`, reports,
  1-vCPU emulation script `crates/opencnpj-lab/scripts/vps-emulate.sh`).
* **Acceptance:** metrics unit-tested on hand-computed examples (including risk–coverage
  and AURC); a trivial baseline (always "count", no entities) produces the expected
  numbers; `--milestone` gating for H-blind.

### WP-3.3 ADV, OOD and informal sets — size M
* **Goal:** 10-SYNTHETIC-DATA.md §6 sets other than H-dev/H-blind.
* **Deliverables:** `eval/adv.jsonl` (≥ 300, minimal pairs and traps), `eval/ood.jsonl`
  (≥ 400; includes imported `OHF-Voice/intents` pt-BR sentences with attribution in
  `eval/DATASET-CARD.md`), `eval/h-informal.jsonl` (≥ 200), `eval/dialogues.jsonl` (≥ 60).
* **Acceptance:** schema validation; each trap of 05-INTERPRETER.md §7 and each SR rule has
  ≥ 5 items; frozen with SHA-256 in the dataset card.

## Phase 4 — Proofs of concept (isolated; nothing user-facing)

### WP-4.1 Experiment A — deterministic interpreter — size L
* **Goal:** 05-INTERPRETER.md in `opencnpj-nlu` with variants A1–A4 behind a runtime
  option; language pack compiler `opencnpj-intel lang compile`; policy of 07 (thresholds
  tuned on S-val + H-dev only).
* **Acceptance:** per-construction tests from `lang/pt-BR/tests/*.yaml`; the worked example
  of 05 §10 passes end to end; full evaluation on S-test, H-dev, ADV, OOD, H-informal
  reported with latency/RSS on the 1-vCPU emulation; error analysis by category
  (entity, intent, role, negation, time, context) in the result file; gate G-A evaluated.

### WP-4.2 Experiment B — classifiers — size M
* **Goal:** 11-MICRO-CLASSIFIER.md §1–§5: B-NB, B-LR, B-SVM, B-PERC, E1-kNN in Rust; B-FT
  via the fastText CLI (dev only); all on delexicalized input from WP-4.1's stages S1–S6.
* **Acceptance:** reproducible training (same manifest → same artifact SHA-256);
  int8-vs-float equivalence test; calibration report; same evaluation and resource
  reporting as WP-4.1.

### WP-4.3 Experiment D — hybrids — size M
* **Goal:** D1 (classifier only when no construction fires) and D2 (combined scores),
  weights tuned on validation.
* **Acceptance:** same reporting; explicit comparison table A vs best B vs D1 vs D2 on
  every slice with confidence intervals (bootstrap over items).

### WP-4.4 Learning curves and diversity — size M
* **Goal:** 12-BENCHMARK-PROTOCOL.md §5.
* **Acceptance:** plots/tables per diversity level with the plateau point; statement
  answering brief §11 "at which point adding examples stops helping".

### WP-4.5 Experiment C — micro neural — size M, time-boxed to one week, only via gate G-C
* **Goal:** 11-MICRO-CLASSIFIER.md §6 at ~10k and ~35k parameters (100k only if the curve
  is still rising).
* **Acceptance:** same reporting as B, plus training time and Rust inference latency;
  explicit answer to "did neural help, and at what cost".

### WP-4.6 Dialogue and context PoC — size M
* **Goal:** 06-CONTEXT.md: acts, inheritance rules, context token (encode, HMAC, size),
  clarification handling, on top of the best interpreter so far.
* **Acceptance:** dialogue scripts of 06 §4 pass; dialogue set metrics (per-turn state
  accuracy, filter leaks = 0); token size distribution (MEASURED); tampered/expired tokens
  rejected (tests).

## Phase 5 — Decision

### WP-5.1 Decision record — size S
* **Goal:** run every variant once on H-blind (`--milestone decision`), write
  `docs/intelligence/results/<date>-decision.md`, update [15-DECISIONS.md](15-DECISIONS.md)
  (ADR-I-007) with the chosen architecture, the rejected ones and why, the measured
  RAM/CPU/storage against 13-RESOURCE-ESTIMATES.md, and open risks.
* **Acceptance:** every NFR in 12-BENCHMARK-PROTOCOL.md §7 is met or explicitly waived with
  a reason; the owner approves in writing (issue comment or chat). **No phase 6 work before
  this approval**, and no implementation PR is opened before it.

## Phase 6 — Product integration (after approval)

### WP-6.1 Production interpreter — size M
* Keep only the chosen variant(s) in `opencnpj-nlu` (losing variants move to `opencnpj-lab`
  or are deleted); language pack embedded in release builds with `--lang-pack` override and
  hot reload; startup and RSS measured.

### WP-6.2 Chat API in `opencnpj serve` — size M
* `POST /api/v1/chat` `{ "message": "...", "context": "..." }` → envelope of 08 §1;
  `POST /api/v1/chat/explain` (returns stages, lattice, matched constructions, signals —
  staff only); route kind `"chat"` in `gate.rs` `kind_of`, with plan quota; metrics per
  stage in `metrics.rs`; logging per 07 §7; response cache keyed by the IR cache key;
  `docs/API.md` and `docs/ACCOUNTS.md` updated; contract tests in
  `crates/opencnpj/tests/chat_contract.rs` on the fixture.
* **Acceptance:** existing tests unchanged and green; chat contract tests cover every
  decision type, privacy switches and quota refusals.

### WP-6.3 Chat node mode — size M
* `opencnpj chat-serve --agg-dir DIR --engine-url URL --engine-key-file F` (no snapshot):
  local aggregates + catalogues + language pack; engine client for lists/lookups; reload
  of aggregates on `current` change; same gate and accounts (local SQLite) or `--auth off`
  behind a trusted proxy; deploy kit additions (`deploy/opencnpj-chat@.service`).
* **Acceptance:** works on the 1-vCPU / 1 GB emulation with 24 months of aggregates;
  engine outage → aggregate answers still served, list questions get a clear "temporarily
  unavailable" answer.

### WP-6.4 Web chat page — size M
* One static page served by the API listener (no external assets, like `admin.html`):
  message box, "Entendi" line, blocks (text, KPI, comparison, table, inline SVG bar/line
  charts with table fallback), clarification buttons, suggestions, CSV download of tables,
  methodology/sources expanders; keyboard accessible; works at phone width.
* **Acceptance:** manual test script with screenshots in the result file; no network calls
  except the chat API.

### WP-6.5 Monthly pipeline and rollback — size M
* After `opencnpj update` switches the snapshot: build aggregates for the new competence →
  `diff-agg` must pass → catalogue check (new TOM/CNAE codes reported) → atomic switch of
  the aggregates pointer; previous kept; `rollback` covers aggregates; integrates with the
  existing auto-update child-process jobs.
* **Acceptance:** end-to-end test on the fixture with two competences; a failing diff blocks
  the switch.

### WP-6.6 Capacity, real VPS and docs — size M
* Load test with `opencnpj loadtest` (chat URLs mix) on the emulation and on a real
  1 vCPU / 1 GB VPS: ≥ 300 connected users with think time, p50/p95/p99, errors, RSS;
  measured CPU-seconds per 1,000 questions and the cost formula of 13 §6; update
  `docs/DEPLOYMENT.md`, `docs/ARCHITECTURE.md`, `README.md`, `CHANGELOG.md`.
* **Acceptance:** NFR-03/04/06 met or waived in 15-DECISIONS.md with numbers.

## Phase 7 — Continuous improvement

* **WP-7.1 Unanswered-question loop:** weekly report of rejected/clarified questions
  (privacy rules of 07 §7); additions to aliases/constructions always come with tests;
  monthly regression run on frozen sets.
* **WP-7.2 Blind-set renewal:** a fresh H-blind every quarter (or after any large
  grammar change), because the previous one leaks into the team's intuition once analysed.
