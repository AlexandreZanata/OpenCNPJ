# 10 — Synthetic pt-BR questions and evaluation sets

Synthetic data serves three purposes: regression tests for the rules, training data for
an optional classifier, and stress tests (noise, negation, context). It never replaces
human-written evaluation data: **the decision is made on human sets**.

## 1. Generator overview

Rust module `synth` in `crates/opencnpj-lab` (research crate, `publish = false`). It
shares the catalogues with the interpreter but **not** the constructions (§7).

```text
families/*.toml ──► frame sampler ──► slot fillers (catalogues) ──► realizer (morphology,
                                                                     contractions, order)
                         │                                              │
                         └── gold IR built from the frame ◄─────────────┘
                                         │
                              register + noise transforms ──► dedupe ──► split ──► JSONL + manifest
```

The gold label is computed from the sampled **frame** (intent, metric, slot values), then
canonicalized with the real IR code. No example is labelled by parsing its own text.

## 2. Families and templates

A *family* is one semantic frame skeleton; it has several surface templates written by
different people/styles. Splits are by family (§5).

```toml
[[family]]
id = "count.stock.sector_place"
frame = { op = "count", metric = "stock", slots = ["sector", "place"], status = "active?" }
templates = [
  "quantas {sector.plural} {exist} em {place}",
  "qual o número de {sector.plural} {status?} em {place}",
  "{place} tem quantas {sector.plural}",
  "me diz quantas {sector.plural} tem {place.prep}",
  "conta pra mim as {sector.plural} de {place}",
  "existem muitas {sector.plural} em {place}",
  "tem quantos {sector.plural} {status?} {place.prep}",
]
exist = ["existem", "tem", "há", "possui"]
```

Realizer rules: gender/number agreement ("quantas padarias", "quantos supermercados",
"as farmácias", "os mercados"); contractions and articles per place (catalogue column:
"em Mato Grosso", "no Paraná", "na Bahia", "em São Paulo", "no Rio de Janeiro"); optional
slots; permitted word-order permutations per template.

Initial family inventory (v1): ~60 families covering every IR op × common metric, plus
dialogue families (follow-ups, corrections, clarification answers) and refusal families.
Target ≥ 8 templates per family, written by at least two authors (human or AI-assisted,
always reviewed by a human).

## 3. Slot fillers

| Slot | Sampling |
|---|---|
| Municipality | Stratified: capitals, large (> 500k), medium, small (< 20k), homonyms, names with articles/hyphens/accents ("Ji-Paraná", "Santa Bárbara d'Oeste"), names that are common words ("Sorriso", "Bonito") |
| UF / region | All 27 UFs in names and codes, regions, "interior de", "capital" |
| Sector | Curated catalogue aliases (singular/plural), CNAE descriptions, raw CNAE codes |
| Period | Every construction of 05-INTERPRETER.md §6 with random anchors |
| Numbers | Digits, "mil", "k", words; population thresholds, top-k, money |
| Porte / MEI / status | All vocabulary entries, including traps ("grandes", "pequenas", "abertas") |

## 4. Linguistic variation and noise

Applied after realization; each example records the transforms applied.

| Transform | Examples |
|---|---|
| Register | formal ("Gostaria de saber o número de…"), neutral, colloquial ("tem quantas…", "me fala aí…", "dá uma olhada em…") |
| Abbreviations | qtas, qnts, qto, vc, pfv, tb, msm, q, p/, c/ |
| Accents | removed entirely, partially, wrong ("cuiába") |
| Typos | ABNT2 keyboard-neighbour substitution, deletion, duplication ("sorrisso"), transposition, phonetic swaps (s/ss/ç/z, x/ch, i/e at word end), split/merged words ("matogrosso", "rio verde" vs "rioverde") |
| Case and punctuation | all caps, no punctuation, extra "??" |
| Fillers | "então", "por favor", "uma dúvida:", "bom dia," |
| Fragments | "padarias sorriso", "farmácias MT por habitante" |
| Compound | two sectors ("padarias e farmácias" → one count per sector, 04-QUERY-IR.md §5), two places ("Sorriso e Sinop" → compare); beware catalogue names that contain "e" ("padaria e confeitaria" is one CNAE description) |
| Negation | "que não sejam MEI", "sem contar filiais", "exceto Cuiabá", "fora as inaptas" |
| Regional/popular forms | "padoca", "boteco", "mercadinho", "borracharia", "lanchonete", "sacolão" (via catalogue aliases, never invented) |

Noise is applied with rates sampled per example (none, light, heavy); certain transform
types are reserved for the test split only (§5) to measure robustness to unseen noise.

## 5. Splits that measure generalization

| Split axis | Rule |
|---|---|
| Template family | Families are assigned to train / validation / test (70/15/15) — test families are never seen in training |
| Slot values | 20 % of municipalities and 20 % of sector aliases appear only in test (entity independence, brief §4) |
| Noise | Two noise types (e.g. phonetic swaps, merged words) appear only in test |
| Dialogue | Dialogue families split the same way |

Deduplication: examples are compared on their **delexicalized normalized form**; at most
3 examples per form per split; cross-split duplicates are removed from train.

## 6. Evaluation sets that are not synthetic

| Set | Size | Who writes it | Use |
|---|---|---|---|
| H-dev | ≥ 300 utterances | Team (owner + collaborators); AI-assisted drafts allowed but marked and reviewed | Error analysis and threshold tuning |
| **H-blind** | ≥ 500 utterances + ≥ 50 dialogues | **People who never saw the templates or constructions** (managers, analysts, friends), collected with a short form (WP-0.3) | Final decision only; frozen; read only by the evaluation harness; results reported in aggregate |
| ADV | ≥ 300 | Team | Minimal pairs (abriram/ativas, padaria/fabricação de pães, sem filiais/só filiais), homonyms, traps of 05 §7, unsupported-but-close questions |
| OOD | ≥ 400 | Team + `OHF-Voice/intents` pt-BR (CC BY 4.0) + chit-chat list | Rejection rate |
| H-informal | ≥ 200 | Team, deliberately informal/typo-heavy | Robustness slice |
| Real usage | grows | Logged unanswered/clarified questions (FR-18, privacy rules) | Continuous improvement; never mixed into H-blind |

**The executor AI must not author H-blind** (it would leak the generator's and the
grammar's style into the test). It prepares the collection form, instructions and
anonymization, and the owner gathers the answers. Each H-blind item gets its gold IR by
two independent annotators; disagreements are resolved and logged.

## 7. Avoiding circular evaluation

* The generator's templates and the interpreter's constructions are written separately
  (different files, ideally different authors), and S-test families are never shown to
  whoever writes constructions.
* The rule interpreter is tuned on H-dev and S-val only; S-test and H-blind are run once
  per milestone and their failures are not individually turned into rules until after the
  decision (then H-blind is renewed with a fresh collection).

## 8. Dataset format and versioning

```json
{"id":"s-000123","text":"qtas padarias tem em sorrisso mt?","gold":{"act":"new_query","decision":"execute","ir":{"op":"count","...":"..."}},
 "meta":{"family":"count.stock.sector_place","template":3,"register":"colloquial","noise":["abbrev","dup_letter"],"split":"test","gen":"synth-0.1.0","seed":42}}
```

Dialogues: `{"id", "turns":[{"text","gold"}…]}` with gold acts and the gold state after
each turn.

Each dataset directory (`/data/opencnpj-data/intelligence/datasets/<dataset-id>/`)
contains the JSONL files and `manifest.json`: generator version and git commit, SHA-256 of
every spec and catalogue file, seed, counts per family/intent/noise/split, diversity
metrics. `dataset-id = <generator version>-<spec hash prefix>-<seed>`. Rebuilding with the
same id must produce byte-identical files (test in CI on a small configuration).

## 9. Diversity metrics (reported in the manifest)

Distinct delexicalized forms per intent; templates per family; slot-value coverage (% of
municipalities/sectors used); token and character n-gram entropy; mean pairwise Jaccard
similarity of delexicalized forms within an intent (lower = more diverse); share of
examples per noise type. Learning-curve studies vary **diversity levels** (L1: 2
templates/family, 25 % of slot values; L2: 5 templates, 50 %; L3: all) as well as **size**
(1k, 3k, 10k, 30k, 100k, 300k), each with 3 seeds (12-BENCHMARK-PROTOCOL.md §5).
