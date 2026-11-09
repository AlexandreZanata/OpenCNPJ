# 04 — Query IR: a typed business query language

The IR (intermediate representation) is the contract between language understanding and
data. Every interpreter variant (rules, classifier, hybrid) must output this IR; every
backend (aggregates, engine, static answers) only accepts it after validation. No
component executes anything that is not a validated IR value.

It is **not** SQL and has no free-form expressions: only the operations, metrics,
dimensions and filters listed here. Unknown combinations are unrepresentable or rejected
by the validator.

Crate: `crates/opencnpj-query` (no I/O, no async, depends on `opencnpj-core` and `serde`).

## 1. Shape

```text
Request  = { op, unit, scope: Filters, time: TimeSpec, output: Output }
op       = Count{metric} | List{order, limit} | Lookup{id} | Compare{dim, members, metric}
         | Rank{dim, metric, dir, k, min_base} | Breakdown{dim, metric} | Trend{metric, grain}
         | Meta{topic}
```

Textual rendering (used in docs, logs and "Entendi:" lines; it is a printer, not a parser
users can type):

```text
COUNT establishments
  WHERE municipality = 9907 (Sorriso/MT) AND sector = bakery@v1 AND status = active
  AT competence 2026-10

COMPARE municipality [9907 Sorriso/MT, 8985 Sinop/MT]
  METRIC per_10k_inhabitants(establishments)  POPULATION ibge_estimate_2025
  WHERE sector = bakery@v1 AND status = active

RANK sector WITHIN uf = MT
  METRIC active_change  PERIOD 2026-04 → 2026-10 (6 snapshots back)
  DIR desc  K 10  MIN_BASE 30
```

## 2. Rust types (normative sketch)

Names are normative; field layout may change during implementation if the invariants in
§3 hold.

```rust
/// Identifiers are dense ids into versioned catalogues, never free text.
pub struct MunicipalityId(pub u16);   // index into the municipality catalogue (RFB/TOM code kept there)
pub struct UfId(pub u8);              // 0..27, plus EX
pub struct RegionId(pub u16);         // IBGE grande região / região intermediária / imediata
pub struct SectorId(pub u16);         // curated sector catalogue entry (versioned)
pub struct CnaeCode(pub u32);         // 7-digit subclass, or a prefix with its level
pub struct Competence(pub u16);       // months since 2000-01 (2026-10 = 321)
pub struct Day(pub u32);              // days since 1900-01-01 (same base as numeric columns)

pub enum Unit { Establishments, HeadOffices }          // SR-06

pub enum Op {
    Count { metric: Metric },
    List { order: ListOrder, limit: u16 },
    Lookup { id: CnpjRef },                          // validated 14- or 8-char CNPJ
    Compare { dim: Dim, members: Members, metric: Metric },   // 2..=4 members
    Rank { dim: Dim, metric: Metric, dir: Dir, k: u8, min_base: u32 },
    Breakdown { dim: Dim, metric: Metric },
    Trend { metric: FlowMetric, grain: Grain },
    Meta { topic: MetaTopic },                       // data date, sources, methodology, help
}

pub enum Metric {
    Stock,                                 // establishments matching the filters in the snapshot
    Flow(FlowMetric),                      // events in a period
    ActiveChange { pct: bool },            // stock(t1) − stock(t0) across snapshots (SR-05)
    Per10k(Box<Metric>, PopulationRef),    // metric / population × 10,000 (Stock only in v1)
    InhabitantsPer(PopulationRef),         // population / stock ("menos concorrentes")
    Share,                                 // stock / stock of the enclosing scope
}

pub enum FlowMetric { Registrations, Closures, NetRegistrations, MeiOptIns, MeiExits }

pub enum Dim { Municipality, Uf, Region(RegionLevel), Sector, Cnae(CnaeLevel), Porte, Status,
               HeadOffice, Mei, Year, Month }

pub enum Dir { Asc, Desc }
pub enum Grain { Month, Year }
pub enum ListOrder { Cnpj, NewestFirst, OldestFirst, CapitalDesc }

pub struct Filters {
    pub place: PlaceFilter,                     // Any | Municipalities(set) | Ufs(set) | Region | Near{..}
    pub place_population: Option<PopRange>,     // "cidades com menos de 50 mil habitantes"
    pub activity: ActivityFilter,               // Any | Sectors(set) | Cnae(set, CnaeScope::Main|Any)
    pub status: StatusSet,                      // bitset over 01,02,03,04,08; default per op (D-02)
    pub porte: PorteSet,                        // bitset over 00,01,03,05
    pub mei: Tri,                               // Any | Yes | No
    pub simples: Tri,
    pub head_office: Tri,                       // matriz only / branches only
    pub natureza: NaturezaSet,                  // codes or group prefixes
    pub opened: Option<DayRange>,               // data_inicio_atividade
    pub status_date: Option<DayRange>,          // data_situacao_cadastral
    pub capital: Option<MoneyRange>,            // reais, company level
    pub exclude: Exclusions,                    // negated places/sectors ("exceto Cuiabá", SR-13)
    pub contact: ContactNeed,                   // lists only: has phone / e-mail
}

pub enum PlaceFilter {
    Any,
    Municipalities(SmallSet<MunicipalityId>),
    Ufs(SmallSet<UfId>),
    Region(RegionId),
    Near { center: MunicipalityId, rule: NearRule },  // Neighbours(depth) | WithinKm(u16)
}

pub struct TimeSpec {
    pub competence: Competence,          // served snapshot unless the user pins one
    pub period: Option<Period>,          // for flows/changes; resolved to explicit days/competences
    pub anchor: Day,                     // reference date of the competence (SR-10)
}

pub enum Period {
    Days(DayRange),                      // flows: inclusive day range
    Competences { from: Competence, to: Competence },  // ActiveChange
}

pub struct Output { pub page: u16, pub with_methodology: bool }
```

`SmallSet<T>` is a sorted, deduplicated inline vector (≤ 8 members inline). Every set is
canonical (sorted, unique) by construction; constructors are the only way to build one.

## 3. Invariants

1. **Canonical form.** Two IR values with the same meaning are byte-equal after
   `canonicalize()`: sets sorted and deduplicated; defaults made explicit (status = active
   for `Stock`, any for flows, D-02); sector sets expanded to CNAE sets *and* the sector id
   kept for display; `Compare` members sorted by id, with the user's order kept separately
   in `Output` for display only; date ranges clipped to the data range; redundant filters
   removed (a municipality filter makes a UF filter on the same UF redundant; a
   contradictory one makes the query invalid, §4).
2. **Explicit time.** Relative expressions never survive parsing: `TimeSpec` holds
   resolved days/competences plus the anchor they were resolved against.
3. **Explicit units and catalogue versions.** The IR records the sector catalogue
   version, municipality catalogue version and population reference it was resolved with.
4. **Closed world.** Every enum is exhaustive; adding a metric or dimension is a code change
   reviewed with its validation rule, templates and tests.

## 4. Validation rules

The validator returns `Ok(ValidRequest)` (a newtype only it can construct) or a list of
`Issue`s. An issue is either **Missing** (a clarification can fix it), **Ambiguous**
(choose among options) or **Impossible** (explain and refuse). Rules (non-exhaustive, all
tested):

| Rule | Kind |
|---|---|
| `Per10k` / `InhabitantsPer` / `place_population` require `dim ∈ {Municipality, Uf, Region}` or a place filter of those, and a population table for the reference year | Impossible / Missing |
| `ActiveChange` requires ≥ 2 competences covering the period in the aggregates store | Impossible → offer `Flow(Registrations)` fallback (SR-05) |
| "Active at a past date" requires a snapshot of that date (SR-02/03) | Impossible → offer alternatives |
| `Trend` requires a flow metric and a period ≥ 2 grains | Missing period → default last 12 months, stated |
| `Rank`/`Compare` on `Sector` require ≥ 2 sectors or the full catalogue as members | — |
| `Compare` needs 2–4 members of the same dimension | Missing / Impossible |
| `status` excludes active while `metric = Stock` of "ativos" | Impossible (contradiction) |
| `mei = Yes` with `porte ⊆ {03, 05}` | Impossible (contradiction; MEI is porte 01 in RFB data) — flagged as "provavelmente vazio", not silently answered |
| `capital` filter with `Unit::Establishments` | Allowed, but the answer notes capital is company level |
| `List` limit ≤ caller plan page size; `contact` filter only for `List` | Impossible / clamp |
| `Lookup` id must pass check-digit validation | Impossible ("CNPJ inválido") |
| Place homonym unresolved (several municipalities, no UF) | Ambiguous (options = UFs) |
| Sector term resolved to > 1 catalogue entry with similar scores | Ambiguous |
| `Near` requires geography assets | Impossible until WP-1.2 delivers them |

## 5. Normalizing equivalent questions

Paraphrase equivalence is achieved in two places:

* **Interpreter** maps surface variation to the same slots (05-INTERPRETER.md).
* **Canonicalizer** removes representational variation (§3.1). Examples that must produce
  byte-equal canonical IR (they are golden tests):

| Question A | Question B |
|---|---|
| "Quantas padarias existem em Sorriso?" | "qtas padarias tem em sorrisso mt" |
| "Compare Sorriso com Sinop" (sector from context) | "Sinop vs Sorriso" |
| "farmácias ativas em Cuiabá" | "drogarias em funcionamento em Cuiabá-MT" (if catalogue maps both to the same CNAE set) |
| "empresas abertas em 2025 em MT" | "quantos CNPJs foram registrados em Mato Grosso no ano de 2025" |

Coordination rule: "X e Y" of the same entity type in a `Count` becomes one value per
member, not the union — "quantas padarias e farmácias tem em Sorriso" → `Compare { dim:
Sector, members: [bakery, pharmacy] }`; "Sorriso e Sinop" → `Compare { dim: Municipality }`.
The union is only produced on explicit request ("somando padarias e farmácias").

And pairs that must **not** be equal: "abriram em 2025" vs "estavam ativas em 2025";
"padarias" (4721-1/01, 4721-1/02) vs "fabricação de pães" (1091-1/0x); "Sorriso" vs
"Sorriso e região"; "sem filiais" vs "só filiais".

## 6. Stable cache keys and reproducibility

* Binary encoding: a hand-written, versioned, little-endian encoding of the canonical IR
  (tag bytes + varints, `opencnpj_core::varint`). Typical size 20–80 bytes **[ESTIMATE]**.
* Cache key = `xxh3_128(encoding_version ‖ canonical_bytes ‖ aggregates_id ‖ catalogue_versions)`.
  The aggregates id embeds the competence and the artifact SHA-256 prefix, so a monthly
  update invalidates naturally (same mechanism as the response cache keys in `gate.rs`).
* `serde` JSON form for debugging, logs, API output (`"query": {...}`) and golden files.
* Every answer carries the canonical JSON and the textual rendering, so it can be re-run
  (`opencnpj-intel run --ir answer.json`) and compared.

## 7. Planning and execution

The planner maps a valid IR to one backend by a static capability table, never by trying
backends in turn:

| IR shape | Backend | Notes |
|---|---|---|
| `Count/Compare/Rank/Breakdown` with `Stock` or `Per10k`, filters within the aggregate's dimensions (place, main CNAE/sector, status = active, porte, MEI, head office) | **Aggregates** | Sum of cells; microseconds to low milliseconds **[HYPOTHESIS]** |
| Same with `Flow` metrics over periods covered by the flow tables | **Aggregates** | |
| `ActiveChange` | **Aggregates** (two stock tables) | |
| `Trend` | **Aggregates** (flow tables) | |
| Any filter outside aggregate dimensions (capital, natureza, CEP, contact, secondary CNAE when not aggregated, status other than the aggregated ones, opened range not aligned to months) | **Engine** (`count` via `search … limit=1` → `total`, or `/stats`) | Lowered to `params::parse` pairs |
| `List`, `Lookup` | **Engine** | Privacy switches and plan limits apply |
| `Meta` | **Static** (manifest, catalogue metadata, methodology texts) | |

Engine lowering example (normative for the mapping, tested against `params::parse`):

```text
COUNT establishments WHERE municipality=9907 AND sector=transport@v1 AND status=active AND opened ≥ 2026-04-12
→ /api/v1/search?municipio=9907&cnae=49,50,51,52,53&situacao=02&data_inicio_atividade_de=2026-04-12&limit=1
  (answer = total)
```

Differential guarantee (NFR-12): for every aggregate-backed IR in the golden set, and for
a seeded random sample of 10,000 IRs per release, `aggregates(ir) == engine(lower(ir))`.
A mismatch fails the monthly build.

## 8. Result model

```rust
pub enum ResultValue {
    Scalar { value: u64 },
    Ratio { num: u64, den: u64, scale: u32 },        // per 10k etc.; formatting is the renderer's job
    Series { points: Vec<(Label, Value)> },          // trends, breakdowns
    Ranking { rows: Vec<(EntityRef, Value, Base)>, excluded_below_min_base: u32 },
    Records { rows: Vec<RecordSummary>, total: u64, next: Option<Cursor> },
    Company(Box<CompanyView>),
    Meta(MetaAnswer),
}

pub struct Answer {
    pub request: ValidRequest,          // canonical IR actually executed
    pub value: ResultValue,
    pub provenance: Provenance,         // competence(s), snapshot SHA-256 prefix, aggregates id,
                                        // population source/year, catalogue versions, CNAE codes used
    pub caveats: Vec<CaveatId>,         // from the caveat catalogue (08-RESPONSES.md)
    pub timing: StageTimings,           // normalize, parse, validate, plan, execute, render (µs)
}
```

## 9. Versioning

* `IR_VERSION` constant; the binary encoding starts with it. A change that alters meaning
  or encoding bumps it; old context tokens (06-CONTEXT.md) are then rejected gracefully
  ("vamos recomeçar a conversa").
* Catalogue versions (`sector@v1`, `municipalities@2026-10`, `population@ibge-2025`) are
  data versions and do not bump `IR_VERSION`.
