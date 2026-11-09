# Work order 2026-11-07 — fix the open review items (PR #5)

You are working in the Rust repository `/home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ`,
branch `docs/intelligence-plan` (pull request #5). Your job is to apply the fixes below,
**exactly as written, one task at a time, in order**. Each task says which files to edit,
what to change, which test to add, how to verify, and the commit message to use.

Background (read only if a task refers to it): `docs/intelligence/results/2026-11-07-review.md`
lists the open problems; this work order turns its items into concrete steps.

## Rules (apply to every task)

1. **Before anything else**, run these and keep the output for the final report:
   ```bash
   cd /home/iiii/PESSOAL-PROJETOS-ALEXANDRE/OpenCNPJ
   git status --short && git branch --show-current && git log --oneline -1
   ```
   The branch must be `docs/intelligence-plan` and the tree must be clean except for this
   file. If not, stop and report.
2. Always build with `export CARGO_TARGET_DIR=/data/opencnpj-target` (the root disk is full).
   Never write build output or data anywhere else.
3. **Do not edit** these crates at all: `crates/opencnpj-core`, `crates/opencnpj-store`,
   `crates/opencnpj-ingest`, `crates/opencnpj`, `crates/opencnpj-desktop`. Never touch
   anything under `/data/opencnpj-data/snapshots/` or `/data/opencnpj-data/raw/`.
4. **Never weaken a test to make it pass** (no deleting assertions, no changing expected
   values, no `#[ignore]`) unless the task tells you to. If a test fails and you cannot fix
   the code within 2 attempts, stop the task, run `git checkout -- .` for that task's files,
   and write in the report what failed (command + output).
5. Golden files (`crates/opencnpj-query/tests/fixtures/ir-v1-golden.jsonl`,
   `crates/opencnpj-insights/tests/fixtures/answers.json`) may only be regenerated when a
   task says so, with the commands given, and you must paste `git diff --stat` of them in
   the report.
6. After each task run the **delivery gate**; all three must pass before committing:
   ```bash
   cargo fmt --all
   cargo fmt --all --check
   cargo clippy --workspace --all-targets --locked -- -D warnings
   cargo test --workspace --locked
   ```
7. Commit only the files of that task (`git add <paths>` — **never** `git add -A` or
   `git add .`), with the exact message given, then push:
   ```bash
   git push origin docs/intelligence-plan
   ```
   Never force-push, never use `--no-verify`, never rebase or amend pushed commits.
8. Code, comments and commit messages in English. Match the style of the surrounding code
   (short doc comments that say *why*).
9. Do not do anything that is not in this work order. If something seems wrong but is not
   listed, write it in the report under "Noticed, not changed".

---

## Task 0 — Record this work order

* `git add docs/intelligence/WORK-ORDER-2026-11-07.md`
* Commit: `docs(intelligence): add the work order for the review fixes`
* Push.

---

## Task 1 — Sector catalogue corrections

File: `crates/opencnpj-insights/catalog/sectors.toml` (array of `[[sector]]` tables with
`id`, `display`, `aliases`, `cnae_core`, `cnae_extended`, `scope`, `since`).

**Important:** a sector's numeric id is its position in the file. Never reorder or delete
sectors. New sectors go **at the end of the file**.

1. `id = "transport"`: keep in `cnae_core` only the codes that start with `49`, `50` or
   `51`. Move every code that starts with `52` or `53` from `cnae_core` to `cnae_extended`
   (keep them sorted ascending). Do not change its aliases.
2. `id = "beauty_salon"`: add `"9602502"` to `cnae_core` (keep sorted). Add these aliases:
   `"salão de beleza"`, `"salões de beleza"`, `"barbearia"`, `"barbearias"`, `"estética"`.
   Skip any alias that is already present.
3. `id = "building_materials"`: move `"4744005"` from `cnae_extended` to `cnae_core`.
4. `id = "grocery"`: add aliases `"mercado"`, `"mercados"`, `"mercadinho"`, `"mercadinhos"`.
5. `id = "real_estate_broker"`: add aliases `"imobiliária"`, `"imobiliárias"`.
6. `id = "accounting"`: add aliases `"contabilidade"`, `"escritório de contabilidade"`,
   `"escritórios de contabilidade"`, `"contábil"`.
7. `id = "law_firm"`: add aliases `"advocacia"`, `"escritório de advocacia"`,
   `"escritórios de advocacia"`.
8. `id = "it_services"`: add aliases `"ti"`, `"tecnologia da informação"`.
9. `id = "truck_freight"`: add aliases `"transporte de carga"`, `"transporte de cargas"`.
10. Append these three sectors at the end of the file, in this order, with the same field
    layout as the existing entries:
    ```toml
    [[sector]]
    id = "school"
    display = { sing = "escola", plural = "escolas", gender = "f" }
    aliases = ["escola", "escolas", "colégio", "colégios"]
    cnae_core = ["8512100", "8513900", "8520100"]
    cnae_extended = ["8511200"]
    scope = "main"
    since = "v1"

    [[sector]]
    id = "civil_construction"
    display = { sing = "empresa de construção civil", plural = "empresas de construção civil", gender = "f" }
    aliases = ["construção", "construção civil", "construtora", "construtoras"]
    cnae_core = [ ...every subclass of divisions 41, 42 and 43, see below... ]
    cnae_extended = []
    scope = "main"
    since = "v1"

    [[sector]]
    id = "passenger_transport"
    display = { sing = "empresa de transporte de passageiros", plural = "empresas de transporte de passageiros", gender = "f" }
    aliases = ["transporte de passageiros", "transporte coletivo"]
    cnae_core = ["4921301", "4921302", "4922101", "4922102", "4922103", "4929901", "4929902", "4929903", "4929904", "4929999"]
    cnae_extended = ["4923001", "4923002"]
    scope = "main"
    since = "v1"
    ```
    For `civil_construction`, get the exact code list with this command and paste all
    codes (as quoted strings, ascending) into `cnae_core`:
    ```bash
    awk -F'\t' 'NR>1 && $2=="subclass" && $1 ~ /^4[123]/ {print $1}' crates/opencnpj-insights/catalog/cnae.tsv | sort
    ```
    It must print 47 codes or fewer; every printed line is 7 digits. If the count is not
    between 30 and 47, stop and report.
11. Update `crates/opencnpj-insights/catalog/cnae_manifest.json`: in `artifacts`, set
    `sectors.toml` → `bytes` and `sha256` to the new file's values:
    ```bash
    wc -c < crates/opencnpj-insights/catalog/sectors.toml
    sha256sum crates/opencnpj-insights/catalog/sectors.toml
    ```
    Do not change any other entry.
12. Validate (all must succeed; paste the output in the report):
    ```bash
    python3 -I -c "import tomllib; s=tomllib.load(open('crates/opencnpj-insights/catalog/sectors.toml','rb'))['sector']; print(len(s)); print([x['id'] for x in s[-3:]])"
    python3 -I -c "
    import tomllib, csv
    codes={r[0] for r in csv.reader(open('crates/opencnpj-insights/catalog/cnae.tsv'), delimiter='\t') if len(r)>1 and r[1]=='subclass'}
    s=tomllib.load(open('crates/opencnpj-insights/catalog/sectors.toml','rb'))['sector']
    bad=[(x['id'],c) for x in s for c in x['cnae_core']+x['cnae_extended'] if c not in codes]
    print('unknown codes:', bad)"
    ```
    The first must print `213` and `['school', 'civil_construction', 'passenger_transport']`.
    The second must print `unknown codes: []`.
13. Gate, then commit `crates/opencnpj-insights/catalog/sectors.toml` and
    `crates/opencnpj-insights/catalog/cnae_manifest.json`:
    `fix(insights): correct sector sets and add missing aliases`.

---

## Task 2 — Density denominators must use the same filters as the numerator

Problem: with a population filter (`place_population`), a place filter or exclusions, the
count (numerator) is filtered but the population (denominator) of a UF or region is not.

File: `crates/opencnpj-insights/src/execute.rs`.

1. Add this public struct near `place_population_for`:
   ```rust
   /// Which municipalities a population sum may include: the same place, population range
   /// and exclusions as the counted establishments, so densities compare like with like.
   pub struct PopulationScope<'a> {
       pub municipalities: Option<&'a BTreeSet<u16>>,
       pub ufs: Option<&'a BTreeSet<u8>>,
       pub range: Option<(u32, u32)>,
       pub exclude_municipalities: &'a BTreeSet<u16>,
       pub exclude_ufs: &'a BTreeSet<u8>,
   }
   ```
   with a method `fn admits(&self, row: &Municipality, reference: PopulationRef) -> bool`
   that returns `false` when: `ufs` is `Some` and the row's UF (`major_uf(row)`) is not in
   it; `municipalities` is `Some` and `row.id` is not in it; `row.id` is in
   `exclude_municipalities`; the row's UF is in `exclude_ufs`; `range` is `Some((low, high))`
   and `population_of(row, reference)` is `None` or outside `low..=high`. Otherwise `true`.
2. Change `place_population_for` to take `scope: &PopulationScope<'_>` instead of
   `municipalities`, `ufs` and `range`, and sum `population_of(row, reference)` over the
   rows where `scope.admits(row, reference)`.
3. Change `member_population_for` to take `scope: &PopulationScope<'_>` instead of
   `municipalities` and `ufs`:
   * `Dim::Municipality`: the row's population if `scope.admits(row, reference)`, else 0.
   * `Dim::Uf` and `Dim::Region(_)`: keep the existing key match, **and** require
     `scope.admits(row, reference)`.
   * other dims: `place_population_for(catalogue, scope, reference)`.
4. Update the two callers in `execute.rs` (`fn place_population` and `fn member_population`)
   to build a `PopulationScope` from `Filters` (`municipalities`, `ufs`, `population`,
   `exclude_municipalities`, `exclude_ufs`).
5. Update the two callers in `crates/opencnpj-lab/src/query.rs` (search for
   `place_population_for(` and `member_population_for(`): build the scope from the request:
   `municipalities`/`ufs` from the existing `resolve_place(...)` result, `range` from
   `request.scope.place_population` exactly as the existing code does, and the exclusions
   from `request.scope.exclude.municipalities` (ids `.0`) and `request.scope.exclude.ufs`
   (ids `.0`) collected into `BTreeSet`s.
6. Add a unit test to the `#[cfg(test)] mod tests` at the end of `execute.rs`:
   write a temporary municipality TSV (use `tempfile`, already a dev-dependency) with the
   exact header of `crates/opencnpj-insights/src/catalog/municipalities.rs` (`const HEADER`)
   and three municipalities: two in one UF with estimate populations 10000 and 200000, one
   in another UF with 60000 (copy the column format of a real row from
   `crates/opencnpj-insights/catalog/municipalities.tsv`, change only id, codes, UF and
   population). Load it with `MunicipalityCatalogue::load` and assert:
   * `member_population_for(.., Dim::Uf, ..)` of the first UF with `range = Some((50_000, 1_000_000))`
     is 200000 (not 210000);
   * the same with no range but the 200000 municipality in `exclude_municipalities` is 10000.
   If you cannot make the TSV load after 2 attempts, stop this test, keep the code change,
   and report it.
7. Gate. Then run the national differential (Task 10 command) and confirm
   `mismatches=0`. Commit `crates/opencnpj-insights/src/execute.rs` and
   `crates/opencnpj-lab/src/query.rs`:
   `fix(insights): filter density denominators like the counts`.

---

## Task 3 — Engine lowering must not widen an empty place or activity

Problem: in `crates/opencnpj-lab/src/query.rs`, `pub fn lower(...)` skips the `municipio`
or `uf` parameter when the resolved set maps to no code, so the engine counts nationwide.

1. In `lower`, after computing `toms`: if `municipalities` is `Some` and `toms` is empty,
   return `anyhow::bail!("place resolves to no municipality")`.
2. After computing `codes` for UFs: if `ufs` is `Some` and `codes` is empty, bail with
   `"place resolves to no UF"`.
3. In both `ActivityFilter` arms: if the activity is not `Any` and `values` is empty, bail
   with `"activity resolves to no CNAE"`.
4. Add a test in the existing `#[cfg(test)]` module of `query.rs` that calls `lower` with a
   request whose place is `PlaceFilter::Municipalities` containing an id that does not
   exist in the catalogue used by the module's other tests, and asserts `lower(...)` is
   `Err`.
5. Gate. Commit `crates/opencnpj-lab/src/query.rs`:
   `fix(lab): refuse lowering when a filter resolves to nothing`.

---

## Task 4 — Check aggregate artifacts before trusting them

File: `crates/opencnpj-insights/src/execute.rs`.

1. At the start of `fn for_each_cell(aggregate, table, ...)`, add:
   ```rust
   // A table missing from the artifact must fail, not read as zero (a current artifact
   // built with `--tables T1,T2` made every flow answer 0).
   if !aggregate.manifest().tables.contains(&table) {
       bail!("aggregate {} has no table {table:?}", aggregate.manifest().competence);
   }
   ```
2. In `AggregateSet::open`, for every historical artifact, add (next to the existing
   competence check):
   ```rust
   if aggregate.manifest().municipality_sha256 != current.manifest().municipality_sha256 {
       bail!("{} was built with another municipality catalogue", path.display());
   }
   ```
3. In `crates/opencnpj-lab/src/query.rs`, in `pub fn diff(` and `pub fn bench(`, right after
   `AggregateSet::open(...)`, add the same check `crates/opencnpj-lab/src/aggregate.rs`
   already does: `ensure!(sha256_file(&args.catalog)? == aggregates.current().manifest().municipality_sha256, "aggregate uses another municipality catalogue")`
   (import `opencnpj_ingest::source::sha256_file` and `anyhow::ensure` if needed).
4. Gate. Run the national differential (Task 10). If it now fails with "another municipality
   catalogue", **do not remove the check**: stop and report. Commit
   `crates/opencnpj-insights/src/execute.rs` and `crates/opencnpj-lab/src/query.rs`:
   `fix(insights): verify aggregate tables and catalogue at open`.

---

## Task 5 — Canonicalize exclusions against the place filter

Problem: golden case `exclusions` keeps São Paulo (UF 25) both in the place filter and in
the exclusions. File: `crates/opencnpj-query/src/ir.rs`.

1. Add `fn canonical_exclusions(scope: &mut Filters, context: &CanonicalContext<'_>) -> Result<(), CanonicalError>`
   and call it in `Request::canonicalize` right after `canonical_place(scope, context)?;`.
   Rules:
   * `PlaceFilter::Ufs(ufs)`: remove from `ufs` every UF in `scope.exclude.ufs`; if `ufs`
     becomes empty return `Err(CanonicalError::ConflictingPlace)`; then clear
     `scope.exclude.ufs` (each entry was either removed from the place or outside it).
     Then remove from `scope.exclude.municipalities` every municipality whose UF
     (`context.municipality_ufs.get(id)`, error `UnknownMunicipality(id)` when missing) is
     not in `ufs` (excluding it changes nothing).
   * `PlaceFilter::Municipalities(ids)`: remove every id that is in
     `scope.exclude.municipalities` or whose UF is in `scope.exclude.ufs`; if empty return
     `Err(CanonicalError::ConflictingPlace)`; then clear `scope.exclude.municipalities` and
     `scope.exclude.ufs`.
   * Any other place: change nothing.
   Rebuild sets with `SmallSet::new(...)` so they stay sorted.
2. Add tests in `crates/opencnpj-query/tests/ir_contract.rs`:
   * canonicalizing golden case `exclusions` gives `place = Ufs([UfId(12)])`,
     `exclude.ufs` empty, `exclude.municipalities = [MunicipalityId(5108)]`;
   * a request with `place = Ufs([25])` and `exclude.ufs = [25]` returns
     `Err(CanonicalError::ConflictingPlace)`.
3. Regenerate the golden files (only now):
   ```bash
   UPDATE_GOLDEN=1 cargo test -p opencnpj-query --test ir_contract golden_requests
   UPDATE_SNAPSHOTS=1 cargo test -p opencnpj-insights --lib golden_snapshots
   git diff --stat crates/opencnpj-query/tests/fixtures crates/opencnpj-insights/tests/fixtures
   ```
   (The first command may report a failure because the old file was compiled in; run the
   gate afterwards.) Only the `exclusions` case may change: exactly one line in
   `ir-v1-golden.jsonl` and only the `"exclusions"` block in `answers.json`. If anything
   else changed, stop and report the diff.
4. Gate. Commit `crates/opencnpj-query/src/ir.rs`, `crates/opencnpj-query/tests/ir_contract.rs`,
   `crates/opencnpj-query/tests/fixtures/ir-v1-golden.jsonl`,
   `crates/opencnpj-insights/tests/fixtures/answers.json`:
   `fix(query): drop exclusions that contradict or repeat the place`.

---

## Task 6 — Evaluation harness: dialogues, ties, AURC

File: `crates/opencnpj-lab/src/eval.rs`.

1. **Dialogues.** `pub fn load_items` fails on `eval/dialogues.jsonl` because its lines are
   `{"id", "turns": [{"text", "gold"}, ...]}`. Change `load_items` so that, for a line with a
   `"turns"` key, it produces one `Item` per turn: `id = format!("{id}#{n}")` (n starts at
   1), `text` and `gold` from the turn, `meta = Value::Null`, `rules = vec![]`, and
   `context = None` for the first turn and `Some(json!({"focus": <previous turn's gold.ir.scope.place>}))`
   for the next turns when the previous turn has a gold IR with `scope.place`. Lines with a
   `"gold"` key keep working as today. Add a test that loads a two-turn dialogue from a
   temporary file and checks the ids `x#1`, `x#2` and that turn 2 has a context.
2. **Ties.** In `fn risk_coverage`, items with the same confidence must form one point:
   after sorting, only push a point at the last item of each run of equal confidence.
   Add a test: input `[(0.9, false), (0.9, true), (0.5, false)]`, total 3 → exactly
   `[(2.0/3.0, 0.5), (1.0, 1.0/3.0)]`.
3. **AURC.** In `fn aurc`, return `area` instead of `area / previous` (no normalization by
   the maximum coverage). The existing hand-computed test must still pass unchanged.
4. Gate. Commit `crates/opencnpj-lab/src/eval.rs`:
   `fix(lab): evaluate dialogues and group equal confidences`.

---

## Task 7 — Synthetic generator noise and sampling

Files: `crates/opencnpj-lab/src/synth.rs`, `crates/opencnpj-lab/synth/pt-BR/families.toml`.

1. `fn abbrev`: replace **whole words only**. Split the text on single spaces, replace a
   word only when it is exactly `quantas`→`qtas`, `número`→`numero`, `você`→`vc`,
   `também`→`tb`, `mesmo`→`msm`, `para`→`p/`, `com`→`c/`; replace the two-word sequence
   `por favor` with `pfv`; join with spaces. ("separa" must stay "separa".)
2. `fn phonetic(text)` → `fn phonetic(text: &str, rng: &mut Rng) -> String`: apply exactly
   **one** change at one position chosen with `rng` among the candidates that exist in the
   text: one `ss`→`s`, one `ç`→`ss`, one `ch`→`x`, or one `s` between two vowels→`z`. If no
   candidate exists, return the text unchanged. Update its caller (`"phonetic" => phonetic(&text, rng)`).
3. `fn typo`: choose `index` only among positions where `chars[index]` and
   `chars[index + 1]` are both alphabetic (`char::is_alphabetic`); if there is none, return
   the text unchanged. (Typos must never change digits; the gold would be wrong.)
4. `families.toml`: replace every `t{place.prep}` with `tem {place.prep}` (12 occurrences:
   check with `grep -o "t{place.prep}" crates/opencnpj-lab/synth/pt-BR/families.toml | wc -l`
   before = 12, after = 0).
5. Coverage levels: in `fn sample`, the closure `sample_index` takes the first
   `coverage %` of the pool (catalogue order, so level L1 only covers the North). Replace
   the selection with an evenly spread subset:
   ```rust
   let limit = (pool.len() * coverage / 100).max(1).min(pool.len());
   let position = rng.below(limit) * pool.len() / limit;
   pool[position].min(table_len - 1)
   ```
6. Add tests: `abbrev("separa para você")` == `"separa p/ vc"`; `typo` on `"abc 2025 xyz"` with
   many seeds never changes `"2025"`; `phonetic` changes at most one place (compare lengths:
   the length difference is at most 1).
7. Gate (the synth determinism test must still pass). Commit
   `crates/opencnpj-lab/src/synth.rs` and `crates/opencnpj-lab/synth/pt-BR/families.toml`:
   `fix(lab): word-level abbreviations, single typos, spread coverage`.

---

## Task 8 — VPS emulation script

Replace the whole content of `crates/opencnpj-lab/scripts/vps-emulate.sh` with:

```bash
#!/usr/bin/env bash
# Run a command on one vCPU with a 1 GiB memory cap. The mode used is printed to stderr so
# results can record it; the command's own exit status is returned unchanged.
set -euo pipefail
cpu="${VPS_CPU:-12}"
memory="${VPS_MEMORY_MAX:-1G}"
props=(-p "MemoryMax=$memory" -p MemorySwapMax=0 -p CPUQuota=100%)
if systemd-run --user --scope -q "${props[@]}" true 2>/dev/null; then
  echo "vps-emulate: mode=user-scope cpu=$cpu memory=$memory" >&2
  exec systemd-run --user --scope -q "${props[@]}" taskset -c "$cpu" "$@"
fi
echo "vps-emulate: mode=sudo-scope cpu=$cpu memory=$memory (user scope unavailable)" >&2
exec sudo systemd-run --scope -q "${props[@]}" taskset -c "$cpu" "$@"
```

Check: `bash -n crates/opencnpj-lab/scripts/vps-emulate.sh` prints nothing, and
`crates/opencnpj-lab/scripts/vps-emulate.sh true; echo $?` prints a `vps-emulate: mode=...`
line and `0` (if it asks for a sudo password, press Ctrl-C and report that instead).
Gate. Commit the script: `fix(lab): run the VPS emulation once and record its mode`.

---

## Task 9 — Clean the frozen evaluation sets (no new items)

**Do not write new questions.** These sets must later be written by people; your job is
only to remove fake coverage and wrong labels. Files: `crates/opencnpj-lab/eval/build-sets.py`
(it regenerates `adv.jsonl`, `ood.jsonl`, `h-informal.jsonl`, `dialogues.jsonl`,
`context.json`, `DATASET-CARD.md`) and `crates/opencnpj-lab/src/eval.rs`.

1. In `build-sets.py`, replace the hard-coded `SECTORS` CNAE lists with the **core** sets
   read from the catalogue, keeping the same names and ids:
   ```python
   import tomllib
   _CATALOG = tomllib.load(open(DIR.parent.parent / "opencnpj-insights" / "catalog" / "sectors.toml", "rb"))["sector"]
   def _core(sector_id):
       for index, entry in enumerate(_CATALOG):
           if entry["id"] == sector_id:
               return index, [int(code) for code in entry["cnae_core"]]
       raise SystemExit(f"unknown sector {sector_id}")
   SECTORS = {
       "padaria": _core("bakery"), "farmácia": _core("pharmacy"), "supermercado": _core("supermarket"),
       "restaurante": _core("restaurant"), "construtora": _core("construction"),
       "academia": _core("gym"), "hotel": _core("hotel"),
   }
   ```
   Add an `assert` that the ids are `13, 144, 182, 164, 43, 87, 95` in that order.
2. Replace every `{"ibge_estimate": 2025}` with `{"ibge_estimate": 2026}`.
3. Delete the loop that appends `" (variante)"` clones (the `for _ in range(1000):` block
   that builds `clone`), and the `while len(informal) < 200:` padding loop.
4. Change `inf(text)` to return `(new_text, applied)` where `applied` lists the
   replacements that actually changed the text (`"abbrev"` for any of them); use that list
   as `meta["noise"]` (use `["none"]` if empty). Remove the `index % 2` labels.
5. After all sets are built and before writing: drop ADV and informal items whose `text`
   was already seen in the same set (keep the first); drop dialogues whose list of turn
   texts was already seen.
6. Homonyms: for every item (and every dialogue turn) whose text contains `Bom Jesus` or
   `Bonito` and does not contain a UF code (`RS`, `MS`, `PI`, `GO`, `PB`, `RN`, `SC`, `RJ`,
   `SP`, `MT`, `BA`, `PE`, `CE`, `MG`, `PR`, `AL`, `SE`, `TO`, `PA`, `AM`, `MA`, `ES`, `RO`,
   `AC`, `AP`, `RR`, `DF`) as a separate word, set `gold = {"decision": "clarify",
   "intent": <same intent>, "act": <same act>, "ir": None}` and `meta["clarify"] = "homonym"`.
7. In the dataset card text the script writes: remove the claim about "capital vs revenue"
   minimal pairs; write the real item counts; add this paragraph near the top:
   "Status: AI-drafted. These sets do not meet WP-3.3, which requires sets written by people
   who never saw the generator or the grammar. They must not be used for the WP-5.1
   decision. See results/2026-11-07-review.md."
8. Regenerate: `python3 -I crates/opencnpj-lab/eval/build-sets.py`. Report the new line
   counts (`wc -l crates/opencnpj-lab/eval/*.jsonl`).
9. In `crates/opencnpj-lab/src/eval.rs`:
   * in the test helper that builds `Capabilities` for the frozen sets (search
     `IbgeEstimate(2025)`), change it to `IbgeEstimate(2026)`;
   * in `frozen_sets_validate_and_cover_rules`, **move** (do not delete) the size minimums
     (`("adv", ADV, 300)` etc.), the `>= 5` rule and trap coverage assertions and the
     `dialogues.len() >= 60` assertion into a new test:
     ```rust
     #[test]
     #[ignore = "WP-3.3 not met: the sets must be rewritten by people (review 2026-11-07)"]
     fn wp_3_3_acceptance() { ... the moved assertions ... }
     ```
     Keep in the original test: every IR validates, OOD items all reject, the dataset-card
     hash check, and each dialogue has at least 2 turns.
10. Gate. Commit `crates/opencnpj-lab/eval/` and `crates/opencnpj-lab/src/eval.rs`:
    `fix(lab): remove cloned items and wrong labels from frozen sets`.

---

## Task 10 — National check, results file, final push

1. Build and run the national differential (takes about a minute):
   ```bash
   cargo build --release -p opencnpj-lab
   OUT=/data/opencnpj-data/intelligence/results/$(date +%F)-work-order-diff.log
   /data/opencnpj-target/release/opencnpj-intel diff-exec \
     --agg /data/opencnpj-data/intelligence/agg/2026-10.ocagg \
     --history /data/opencnpj-data/intelligence/backfill/aggregates \
     --snapshot /data/opencnpj-data/snapshots/2026-10-balanced-1.2.ocnpj \
     --catalog crates/opencnpj-insights/catalog/municipalities.tsv \
     --sectors crates/opencnpj-insights/catalog/sectors.toml \
     --cases crates/opencnpj-query/tests/fixtures/ir-v1-requests.jsonl > "$OUT" 2>&1; echo "exit=$?"
   cat "$OUT"
   ```
   Expected: `exit=0`, last line `summary compared=... mismatches=0 ...`, and
   `estavam_ativas_2025_mt` still `557339`.
2. Create `docs/intelligence/results/<today>-work-order-fixes.md` (use `date +%F`) with:
   title, the commit list (`git log --oneline fda63d7..HEAD`), per task one line of what
   changed, the exact diff-exec command above and its **full raw output** in a code block,
   and a "Not done / stopped" section. Do not edit `2026-11-07-review.md`.
3. Gate. Commit the results file: `docs(intelligence): record the work order results`.
   Push.

---

## Final report (print this at the end, it will be reviewed)

```
## Work order report
Start: <branch, first commit, git status output>
### Task N — <title>
Status: done | stopped
Files changed: <list>
Commands run and key output: <gate result lines, validation outputs, diff --stat when asked>
Commit: <hash> <message>
### Noticed, not changed
<anything odd you saw but did not touch>
### Final state
git log --oneline fda63d7..HEAD
git status --short
Last gate: fmt / clippy / test results (pass/fail counts)
diff-exec summary line
```
