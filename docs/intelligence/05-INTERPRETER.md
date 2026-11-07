# 05 — Deterministic interpreter design

Design of the rule/automaton interpreter (experiment A, and the deterministic core of
the hybrid D). It turns one utterance into scored frame candidates and then into a
validated IR ([04-QUERY-IR.md](04-QUERY-IR.md)). Context handling is in
[06-CONTEXT.md](06-CONTEXT.md); acceptance/clarification policy in
[07-CONFIDENCE-AND-SAFETY.md](07-CONFIDENCE-AND-SAFETY.md).

Crate: `crates/opencnpj-nlu` (pure library; no I/O except loading its compiled language
pack; no async).

## 1. Principles

1. **Entities first, then intent.** Places, sectors, sizes, statuses, numbers and dates are
   recognized from closed catalogues and replaced by typed symbols. Intent recognition then
   sees "`quantas <SECTOR> tem em <MUNI> <UF>`" and is independent of which city or sector was
   named (brief §4). Experiment A2 compares this with joint decoding over an entity
   lattice (§9).
2. **Data, not code.** Vocabulary, synonyms, abbreviations and grammar constructions live
   in declarative files compiled into one immutable language pack. No `if/else` chains per
   phrase.
3. **One pass, bounded work.** Every stage is linear in the input length with bounded
   constants; fuzzy matching is restricted, bounded and only used for unknown tokens.
4. **Keep alternatives until the grammar decides.** Ambiguous tokens ("rio", "se", "mt",
   "abertas", "menos") produce several candidate readings; the constructions and the
   validator pick one or ask.

## 2. Pipeline

```text
utterance (≤ 500 chars)
 S0 guard          reject control chars, over-length; keep original for offsets
 S1 normalize      fold accents/case with the core folding table → lowercase ASCII; keep digits
                   and the separators that carry meaning ( / - % , ? . ); collapse spaces
 S2 tokenize       words, numbers ("50 mil", "50.000", "50k"), CNPJ-shaped strings (validated
                   with opencnpj-core), dates ("03/2025", "2025-03", "março de 2025")
 S3 rewrite        abbreviations and colloquial forms → canonical tokens (qtas→quantas,
                   qnts→quantos, pra→para, pro→para o, num→em um, vc→você, tb→também);
                   number words → numbers ("seis"→6, "dois mil e vinte e cinco"→2025)
 S4 spot           longest-match over the phrase lexicon (all gazetteers in one FST);
                   fuzzy fallback for unknown tokens (§5); output: lattice of (span, symbol, payload)
 S5 time/number    small deterministic grammar for periods and quantities (§6)
 S6 delexicalize   symbol sequence, e.g. [Q_HOWMANY SECTOR V_HAVE P_IN MUNI UF Q]
 S7 constructions  one multi-pattern automaton over the symbol alphabet → evidence:
                   intent votes, slot-role bindings, modifiers (negation scope, metric, order)
 S8 scorer         optional statistical intent scorer over the delexicalized sequence
                   (experiments B/D; 11-MICRO-CLASSIFIER.md)
 S9 frame          best frame = argmax evidence (+ scorer); slots filled by type and role;
                   dialogue act and context merge (06-CONTEXT.md)
 S10 IR            build → validate → canonicalize (04-QUERY-IR.md)
 S11 policy        execute | clarify | reject (07-CONFIDENCE-AND-SAFETY.md)
```

## 3. Language pack (declarative sources → one compiled artifact)

Sources live in Git (small text files; they are code-reviewed data):

```text
crates/opencnpj-nlu/lang/pt-BR/
  normalize.toml        abbreviations, elisions, number words, ordinals, spelling variants
  function_words.txt    known non-entity words (prevents fuzzy-matching "tem" to a city)
  cues.toml             cue phrases → symbol (Q_HOWMANY, Q_LIST, RANK_MORE, PER_CAPITA, …)
  places.toml           curated place aliases (sampa, bh, floripa, "região de", UF names/codes, regions)
  dims.toml             porte, situação, matriz/filial, MEI/Simples, natureza groups vocabulary
  constructions/*.rules intent and role constructions (§7)
  tests/*.yaml          per-construction positive/negative examples (run in cargo test)
generated (not hand-edited; produced by WP-1.2/1.3 from the catalogues):
  municipalities.tsv    id, IBGE code, TOM code, UF, display name, folded name, variants
  sectors.tsv           sector id, aliases (singular/plural), CNAE set, catalogue version
```

Compiler (`opencnpj-intel lang compile`) produces `lang-pt-BR-<version>.bin`:

| Part | Structure | Why |
|---|---|---|
| Rewrite map | `fst::Map` token → rewrite id | O(len) lookup, shared prefixes |
| Phrase lexicon | `fst::Map` over folded phrases (tokens joined by a space) → payload index | Exact and fuzzy (`fst::automaton::Levenshtein`) lookups on the same structure |
| Phonetic index | `fst::Map` over pt-BR phonetic keys of place/sector words → payload index | Catches "sorrisso", "cuiaba", "xinop"-type spellings at key distance 0 (§5) |
| Payloads | contiguous array of `(symbol u8, kind u8, id u16, flags u16, prior u16)` | Cache-friendly, no pointers |
| Constructions | multi-pattern automaton over symbol bytes + rule table (rule id → actions) | All constructions matched in one pass |
| Metadata | versions, source SHA-256s, symbol table | Reproducibility, rollback |

The pack is loaded once (`include_bytes!` for the default pack, or memory-mapped from
`--lang-pack` for hot updates) and shared as `Arc<LangPack>` across threads; all lookups
are read-only. Expected size well under 5 MB **[ESTIMATE]**: ~5,600 municipalities × ~2
variants + ~3,000 sector aliases + ~1,500 cue/dimension phrases ≈ 20k phrases; an FST of
20k short keys is typically a few hundred KB.

Automaton choice: the symbol alphabet is < 256, so each symbol is one byte and the
constructions are byte regexes. Candidates: `regex-automata` (multi-pattern, pattern ids,
serializable dense DFAs that deserialize zero-copy from bytes) or a hand-written token
trie with wildcards. WP-3.2 picks one by measurement; adding `regex-automata` is
acceptable (small, no runtime dependencies beyond its own family, widely used) if it wins.

## 4. Symbol inventory (v1 draft)

| Group | Symbols | Example surface forms |
|---|---|---|
| Question | `Q_HOWMANY`, `Q_LIST`, `Q_WHICH`, `Q_WHERE`, `Q_COMPARE`, `Q_HOW`, `Q_WHEN`, `Q_DETAIL`, `Q` (question mark) | quantas, qual o número de, tem quantos, conta pra mim · me mostra, liste, quero ver, relação de · quais · onde · compare, comparar, versus, vs, x · como (está, evoluiu) · desde quando · dados do CNPJ |
| Ranking | `RANK_MORE`, `RANK_LESS`, `TOP_K`, `ORDER_NEW`, `ORDER_OLD` | mais, maior(es), top 10, líderes · menos, menor(es) · mais recentes, novas · mais antigas |
| Metric | `PER_CAPITA`, `GROWTH`, `SHARE`, `COMPETITORS` | por habitante, per capita, densidade, por 10 mil habitantes · crescendo, cresceu, ganhou, aumentou, evolução, variação · participação, % do total · concorrentes, concorrência |
| Event | `EV_OPEN`, `EV_CLOSE`, `ST_ACTIVE`, `ST_INACTIVE` | abriram, surgiram, novas, registradas, criadas · fecharam, encerradas, baixadas, deram baixa · ativas, em funcionamento, funcionando · inativas, paradas |
| Existence | `V_EXIST`, `V_HAVE` | existem, há, tem, possui |
| Unit | `U_ESTAB`, `U_COMPANY`, `U_CNPJ`, `U_HQ`, `U_BRANCH` | estabelecimentos · empresas, negócios, firmas · CNPJs · matrizes, sedes · filiais |
| Entities | `MUNI`, `UF`, `REGION`, `PLACE_KIND`, `SECTOR`, `CNAE_CODE`, `CNPJ`, `PORTE`, `MEI`, `SIMPLES`, `NATUREZA` | Sorriso · MT, Mato Grosso · Centro-Oeste · cidades, municípios, estados · padarias · 4721-1/02 · 33.000.167/0001-01 · micro, pequeno porte · MEI · Simples · associação |
| Quantities | `NUM`, `POP_UNIT`, `PERIOD`, `DATE`, `YEAR` | 50, 50 mil · habitantes · últimos seis meses · março de 2025 · 2025 |
| Relations | `P_IN`, `P_OF`, `P_NEAR`, `P_WITH`, `P_BETWEEN`, `CMP_LT`, `CMP_GT`, `CONJ_AND`, `CONJ_OR` | em, no, na · de, do, da · perto de, próximas de, vizinhas de, região de · com · entre · menos de, abaixo de · mais de, acima de · e · ou |
| Discourse | `NEG`, `EXCEPT`, `ONLY`, `ALSO`, `NOW`, `AND_WHAT`, `ANAPHOR`, `CORRECT`, `RESET`, `THANKS`, `HELP`, `YES`, `NO` | não, sem · exceto, menos (before an entity), fora, tirando, desconsiderando · só, apenas, somente · também · agora · e (utterance-initial), e em · dessas, desses, delas, nesse setor, nessa cidade · na verdade, quis dizer, corrigindo · esquece, nova pergunta · obrigado · ajuda, o que você faz · sim · não |
| Unknown | `UNK`, `UNK_CAP` | unknown token, capitalized unknown (possible name) |

Each symbol is one byte in the delexicalized sequence; entity symbols carry their
payloads on the side.

## 5. Entity spotting and approximate matching

**Exact phrases.** At each token position, look up the longest phrase (≤ 6 tokens) in the
phrase FST. "material de construção" wins over "construção"; "mato grosso do sul" over
"mato grosso". Plurals and gender variants are generated at compile time ("padaria",
"padarias"; "farmácia", "farmácias", "drogaria"), so no stemming runs at query time.

**Ambiguous short forms.** Two-letter UF codes that are also words (`se`, `to`, `pa`, `ma`,
`al`, `es`, `ce`, `am`, `pe`, `go`, `ac`, `pr`) and the colloquial `mt` (= "muito") are
tagged UF only when (a) written in uppercase in the original text, or (b) adjacent to a
place (after `MUNI`, after `em/no/na/de/do/da`, after "estado de"), or (c) they are the
whole utterance in a clarification answer; otherwise they stay words. City names that
are common words ("Rio", "Natal", "Bonito", "Palmas", "Vitória", "Esperança", "Salvador",
"Sorriso", "Formosa") need a place context (preposition, UF next to them, capitalization,
or an expected `PLACE` slot from a pending clarification); without it they enter the
lattice with both readings and a low prior.

**Fuzzy matching** (only for tokens not found exactly and not in `function_words.txt`):

1. Phonetic key lookup (pt-BR orthographic normalization: `ss→s`, `ç→s`, `c[e,i]→s`,
   `qu→k`, `ch→x`, `lh→li`, `nh→ni`, `ph→f`, `y→i`, `w→v`, silent initial `h`, doubled
   letters collapsed). The exact rule set is chosen by experiment (BuscaBR-style keys vs
   plain folding), see 12-BENCHMARK-PROTOCOL.md.
2. Levenshtein automaton over the phrase FST: distance 1 for 4–7 characters, 2 for ≥ 8,
   plus adjacent transpositions generated explicitly (same policy as the name search in
   `crates/opencnpj-store/src/search.rs`). Bounded: at most 16 candidates per token.
3. Candidate score = edit cost + log prior (municipality population; sector usage
   frequency) + context agreement (a UF token next to it that matches the candidate's UF;
   an expected slot type). Auto-accept only when the best candidate is unique, the token
   has ≥ 5 characters and the margin to the next candidate exceeds a threshold tuned on
   the validation set; otherwise the frame keeps the candidates and policy asks
   "Você quis dizer Sorriso (MT)?".

**Homonyms.** "Bom Jesus" maps to five municipalities. Resolution order: explicit UF in
the utterance → UF or region in the dialogue context → single candidate inside the
query's place scope ("em MT") → clarification listing the UFs. Never by population prior
alone.

**CNAE codes and CNPJs.** Patterns like `4721-1/02`, `4721102`, `47.21-1-02` resolve to
codes in the CNAE catalogue; CNPJ-shaped strings are validated by check digit before
becoming a `CNPJ` symbol (invalid → the policy explains the CNPJ is invalid).

## 6. Time and quantity grammar

A small finite grammar (compiled with the constructions) turns time expressions into
`Period` values relative to the **data anchor** (the served snapshot's reference date,
SR-10), never the wall clock:

| Expression | Resolution (anchor 2026-10-11, competence 2026-10) |
|---|---|
| "em 2025", "no ano de 2025" | 2025-01-01 … 2025-12-31 |
| "este ano", "neste ano" | 2026-01-01 … 2026-10-11 (partial year, stated) |
| "ano passado" | 2025-01-01 … 2025-12-31 |
| "últimos seis meses", "nos últimos 6 meses", "últimos 180 dias" | flows: 2026-04-12 … 2026-10-11; stock change: competences 2026-04 → 2026-10 |
| "desde março", "a partir de março de 2025" | 2025-03-01 … anchor (month without year = the latest past March) |
| "entre 2022 e 2024" | 2022-01-01 … 2024-12-31 |
| "primeiro semestre de 2024", "1º tri de 2025" | half-year / quarter ranges |
| "recentemente", "recentes" | ambiguous → default last 90 days **and** state it; configurable |

Quantities: "50 mil", "50.000", "50k", "cinquenta mil" → 50,000; comparators `CMP_LT`
("menos de", "abaixo de", "até") and `CMP_GT` ("mais de", "acima de", "pelo menos") bind
to the following `NUM` and to the next quantity noun (`POP_UNIT` habitantes → population
filter; `U_ESTAB` → minimum base; "reais", "R$", "mil reais" → capital).

## 7. Constructions (the grammar)

A construction is a pattern over symbols plus actions. Constructions are **local**: they
recognize fragments that carry intent or role evidence, not whole sentences, so word
order, filler words and extra phrases do not break them. Draft syntax:

```text
# constructions/count.rules
rule count.howmany
  match   Q_HOWMANY (U_ESTAB|U_COMPANY|U_CNPJ)? ACT
  vote    count +3
rule count.exist_inverted              # "em Sorriso tem quantas padarias"
  match   PLACE V_HAVE Q_HOWMANY ACT
  vote    count +3
rule rank.which_place_more             # "qual cidade tem mais farmácias"
  match   (Q_WHICH|Q_WHERE) PLACE_KIND? (V_HAVE|V_EXIST)? RANK_MORE ACT?
  vote    rank +4 ; set dir=desc ; set dim=from(PLACE_KIND, default=municipality)
rule metric.per_capita
  match   PER_CAPITA
  set     metric=per10k(stock)
rule role.near
  match   PLACE_KIND? P_NEAR MUNI
  bind    place.near.center=MUNI
rule role.population_lt
  match   CMP_LT NUM POP_UNIT
  bind    place_population.max=NUM
rule role.except
  match   (EXCEPT|NEG) (MUNI|UF|SECTOR|PORTE|MEI|U_BRANCH)
  bind    exclude+=$2
macro ACT   = SECTOR (CONJ_AND SECTOR)* | CNAE_CODE | U_ESTAB | U_COMPANY
macro PLACE = MUNI UF? | UF | REGION
```

Matching runs once over the symbol sequence with all patterns (overlapping matches
reported with their rule id). The frame resolver:

1. sums votes per intent; the top intent needs an absolute minimum and a margin over the
   second (both tuned on validation, 07-CONFIDENCE-AND-SAFETY.md);
2. applies `set`/`bind` actions of the matched constructions; conflicting bindings for
   the same slot are an ambiguity;
3. fills remaining slots by type (an unbound `SECTOR` becomes the activity filter; an
   unbound `MUNI` the place filter; several `MUNI` with `Q_COMPARE` the compare members);
4. records which tokens were *explained*; unexplained content tokens lower confidence
   (§8 of 07).

Disambiguation constructions for known traps (each has golden tests):

| Trap | Rule |
|---|---|
| "abertas" = active or newly registered | With a `PERIOD`/`YEAR`/`DATE` → `EV_OPEN` (registrations); without → `ST_ACTIVE` |
| "menos" = rank ascending, comparator or exclusion | `menos de NUM` → comparator; `menos` + entity → exclusion; `RANK_LESS` otherwise |
| "empresas grandes" | `PORTE_BIG` → ambiguity (porte "demais" or capital threshold), SR-08 |
| "fecharam" | `Flow(Closures)` = baixadas (SR-04); "faliram" → closures + caveat that falência is a motivo, not tracked separately in v1 |
| "crescendo" without period | `ActiveChange` over the last 6 available competences, stated (SR-05) |
| "mais" in "mais de 50 mil habitantes" vs "mais farmácias" | `CMP_GT NUM` binds first; leftover `RANK_MORE` votes rank |

## 8. Complexity and memory

| Stage | Time | Allocation |
|---|---|---|
| S1–S3 | O(chars) | one reusable buffer per worker (thread-local scratch) |
| S4 exact | O(tokens × 6) FST lookups (each O(key length)) | lattice in a `SmallVec` |
| S4 fuzzy | only unknown tokens; bounded automaton walk, ≤ 16 candidates | bounded |
| S5–S7 | O(symbols) automaton pass; ≤ 64 symbols | none |
| S9–S10 | O(slots) | IR uses inline small sets |

Expected interpretation time on one slow core: tens of microseconds for typical questions
**[HYPOTHESIS]**; the 10 ms requirement leaves two orders of magnitude of margin. The
resident cost is the language pack (< 5 MB) plus per-worker scratch (< 64 KB).

## 9. Variants to evaluate (experiment A)

| Variant | Difference |
|---|---|
| A1 entities-first | Best single entity reading per span before constructions |
| A2 lattice | Keep up to 3 readings per ambiguous span; constructions choose (beam ≤ 8 paths) |
| A3 no-phonetic | Fuzzy via Levenshtein only (measures the value of the phonetic index) |
| A4 strict | Fuzzy disabled (lower bound for typo robustness) |

## 10. Worked example

`"Qtas padarias tem em sorrisso mt?"`

```text
S1  qtas padarias tem em sorrisso mt ?
S3  quantas padarias tem em sorrisso mt ?
S4  padarias → SECTOR(bakery@v1)        exact (generated plural)
    sorrisso → MUNI{Sorriso/MT}         phonetic key "sorizo" = key("sorriso"); Levenshtein 1
    mt       → UF(MT)                   after MUNI (rule (b)); agrees with Sorriso/MT → boost
S6  Q_HOWMANY SECTOR V_HAVE P_IN MUNI UF Q
S7  count.howmany (+3) → intent count; role: place = MUNI (UF absorbed: same UF)
S10 COUNT establishments WHERE municipality=Sorriso/MT AND sector=bakery@v1 AND status=active AT 2026-10
S11 execute (intent margin high; one fuzzy entity with unique candidate, UF agreement)
"Entendi: estabelecimentos ativos · Padaria (4721-1/01, 4721-1/02) · Sorriso/MT · base RFB 2026-10"
```
