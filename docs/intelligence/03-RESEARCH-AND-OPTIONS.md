# 03 — Research: architectures for closed-domain pt-BR understanding

This document compares the candidate approaches, states what published work says, and
lists the hypotheses our own experiments must settle. **Nothing here is a measured result
on our task**; the qualitative scores in §4 are priors to be replaced by the benchmark
results (12-BENCHMARK-PROTOCOL.md).

## 1. The problem, precisely

* **Output space is small and typed:** ~10 operations, ~12 metrics, ~10 dimensions,
  ~15 filter types (04-QUERY-IR.md). The hard part is mapping wide surface variation onto
  it, not representing meaning.
* **Entities are closed and large:** 5,572 municipalities (241 homonym names), 27 UFs,
  ~1,359 CNAE codes plus ~200 curated sectors and thousands of activity phrases. They are
  *looked up*, not learned.
* **Utterances are short** (typically 4–20 tokens), noisy (no accents, typos,
  abbreviations, regional forms) and often elliptical in follow-ups.
* **Precision dominates:** a wrong number is worse than a clarification (NFR-07).
* **Resources are tiny:** 1 vCPU, 1 GB, interpretation p95 < 10 ms, artifacts < 5 MB.

Consequence: the decisive capabilities are (1) robust entity spotting, (2) intent and
role assignment that does not depend on which entity was named, (3) reliable abstention.

## 2. Published evidence that applies

| Work | What it shows | Relevance |
|---|---|---|
| Snips Voice Platform NLU (Coucke et al., 2018, arXiv:1805.10190) | Embedded, private NLU: a **deterministic regex parser runs first**; only if it finds nothing, a **logistic-regression intent classifier** and **one linear-chain CRF per intent** for slots (features: n-grams, word shape, gazetteers, built-in entities from Rustling — a Rust port of Duckling — word clusters, feature dropout). NLU models of a few hundred KB to a few MB; NLU latency 1.26 ms (MacBook Pro), 2.12 ms (iPhone 6s), 60.3 ms (Raspberry Pi 3), 220 ms (Pi Zero). | The closest precedent: hybrid, linear, gazetteer-heavy, on-device. Our entities are even more closed, so CRFs may be unnecessary. |
| fastText, "Bag of Tricks for Efficient Text Classification" (Joulin et al., 2016, arXiv:1607.01759) | Linear models over averaged word/n-gram embeddings with hashing match deep models on many text-classification tasks at orders of magnitude lower cost. | Baseline B-fastText; also the design of a tiny "neural" option that is really linear. |
| FastText.zip (Joulin et al., 2016, arXiv:1612.03651) | Product quantization + pruning + hashing: models often < 100 kB; ~0.8 % / 1.7 % accuracy loss at 64 KiB / 32 KiB on their benchmarks. | Quantization path if a classifier is kept; size budget is not a constraint. |
| Feature hashing (Weinberger et al., 2009, "Feature Hashing for Large Scale Multitask Learning") | Hashing sparse features into a fixed vector keeps linear models compact with bounded collision loss. | B variants without a stored vocabulary. |
| Duckling (Facebook) / Rustling (Snips) | Rule-based extraction of times, numbers, quantities, robust in production. | Our time/quantity grammar (05-INTERPRETER.md §6) follows this style, specialised to pt-BR and data-anchored dates. |
| Phoenix and other semantic-grammar parsers (ATIS era) | Hand-written domain grammars with robust fragment matching handle spontaneous speech in closed domains. | Constructions are *local* fragments, not full sentences — the robustness trick. |
| Guo et al. 2017; Hendrycks & Gimpel 2017; Chow 1970; Geifman & El-Yaniv 2017 | Modern networks are often miscalibrated; max-softmax is a baseline OOD signal; reject option theory; risk–coverage selection. | Confidence policy (07-CONFIDENCE-AND-SAFETY.md). |

Not found / not usable: no permissively licensed pt-BR intent dataset for business
statistics; Amazon MASSIVE has pt-PT only (CC BY 4.0); MultiATIS++ data is LDC-licensed.
pt-BR smart-home sentences (`OHF-Voice/intents`, CC BY 4.0) are usable as out-of-domain
negatives.

## 3. Candidate architectures

### A — Deterministic (semantic grammar over automata)

Gazetteers in an FST with bounded fuzzy matching, a pt-BR time/quantity grammar,
delexicalization to a symbol sequence, and local constructions in one multi-pattern
automaton that vote for intents and bind roles (05-INTERPRETER.md). Strengths: exact
control, explainable, no training data needed, microsecond latency, trivially small.
Weaknesses: paraphrases not anticipated by any construction fall through; maintenance
grows with the number of constructions; scores are not probabilities.

### B — Small statistical classifier (on delexicalized text)

Input: the symbol/word sequence after entity replacement ("quantas `<SECTOR>` tem em
`<MUNI>`"), so the model never sees city or sector names. Features: word unigrams/bigrams,
character 3–5-grams (typo robustness for cue words), symbol n-grams; hashed to 2^16–2^18
buckets. Models: multinomial Naive Bayes, logistic regression (L2 or elastic net),
linear SVM (one-vs-rest), averaged perceptron, fastText-style averaged embeddings.
Optional hierarchy: act (new / follow-up / clarification answer / OOD) → intent → metric.
Roles still come from rules or a tagger.

### C — Micro neural model trained from scratch

Character-CNN or a tiny GRU over symbols + characters, multitask heads (intent, metric,
act), int8 weights, inference hand-written in Rust. Candidate budgets 10k / 50k / 100k /
500k / 1M parameters. Only pursued if B misses its gate (§6).

### D — Hybrid

A for entities, numbers, dates and roles; B (or C) as the intent/act scorer; votes and
scores combined; validator and policy decide. Precedent: Snips (deterministic first,
statistical second). Variants: D1 "rules first, classifier only when no construction
fires"; D2 "always combine" (log-linear sum of rule votes and classifier log-probs with
weights tuned on validation).

### E — Others considered

| Option | Verdict |
|---|---|
| E1 Example-based nearest neighbour (hashed char n-gram vectors or BM25 over delexicalized exemplars; intent of the nearest exemplars; distance as OOD signal) | **Test** as a B variant: maintenance by adding examples, natural OOD score, tiny |
| E2 Weighted finite-state transducers (OpenFst-style) | Powerful but brings heavy C++ tooling; our automaton + scores covers the need |
| E3 CRF/sequence tagger for slot roles | Only if role-assignment errors dominate the error analysis |
| E4 Reversible grammar (one grammar both generates training data and parses) | Attractive for maintenance, but **evaluation becomes circular**: the parser trivially covers its own generator. Allowed only with template-family hold-outs and the blind human set |
| E5 Transformers, generic embeddings, vector databases | Excluded unless evidence demands it: even small multilingual encoders are ~10⁸ parameters (hundreds of MB, tens of ms per query on one slow core), against NFR-04/05/10 |

## 4. Comparison matrix (priors, to be replaced by measurements)

Scale: ●●● strong, ●● medium, ● weak. Resource columns are order-of-magnitude
**[ESTIMATE]**s for our sizes.

| | Accepted precision | Unseen paraphrases | Typos (cue words) | OOD rejection | Calibrated scores | Interp. latency (1 slow core) | Artifact | Training data needed | Maintenance per new intent |
|---|---|---|---|---|---|---|---|---|---|
| A rules | ●●● | ● | ●● (fuzzy on lexicon) | ●● (domain-evidence gate) | ● (needs mapping) | 10–100 µs | < 2 MB | none (tests only) | constructions + tests |
| B-NB | ●● | ●● | ●● (char n-grams) | ● | ● | < 50 µs | < 1 MB | synthetic + human | examples |
| B-LR / SVM | ●●● | ●● | ●● | ●● (with OOD class + feature mass) | ●● (temperature/isotonic) | < 50 µs | 0.1–3 MB | synthetic + human | examples |
| B-fastText | ●●● | ●● | ●●● | ●● | ●● | < 100 µs | 0.05–5 MB | synthetic + human | examples |
| C char-CNN/GRU | ●●● | ●●● (hypothesis) | ●●● | ●● | ● (needs calibration) | 0.1–2 ms | 0.05–2 MB int8 | more, and tuning | examples + retraining discipline |
| D hybrid | ●●● | ●●● | ●●● | ●●● | ●● | A + B | A + B | synthetic + human | constructions or examples |
| E1 kNN | ●● | ●● | ●●● | ●●● (distance) | ● | 0.1–1 ms | 1–5 MB | exemplars | exemplars |

## 5. The brief's questions (§12), answered as testable hypotheses

1. **Why would a deterministic algorithm not suffice?** H1: entity extraction and the
   frequent question shapes are fully covered by A; residual errors concentrate in rare
   paraphrases and dialogue acts. Test: error analysis of A on H-dev/H-blind by category.
2. **What error rate justifies a classifier?** Gate G-B (§6): A's coverage on H-blind
   < 85 % at ≥ 99 % accepted precision, or ≥ 30 % of A's misses being intent/act errors
   (not entity errors).
3. **Does a classifier generalize to unseen phrasing?** H2: on template families held out
   from training, B-LR improves intent accuracy over A by ≥ 5 pp. Test: E-B vs E-A on S-test
   (family hold-out) and H-blind.
4. **Bytes and cycles added?** Measured per variant: artifact bytes, RSS delta, p50/p95/p99
   µs on the 1-vCPU emulation (12-BENCHMARK-PROTOCOL.md).
5. **Does synthetic training help?** H3: gains plateau before 100k examples; diversity
   (template families, slot coverage) matters more than count. Test: learning curves ×
   diversity levels.
6. **Could the same gain come from better rules/dictionaries?** Test: "rule-improvement
   budget" — give A the same engineering time used to build B (time-boxed, logged), then
   re-measure on the untouched H-blind.
7. **Behaviour on informal pt-BR?** Dedicated H-informal and ADV slices (no accents, typos,
   abbreviations, regionalisms) reported separately for every variant.
8. **Maintenance cost as intents grow?** Simulated: add two new intents (e.g. "Simples
   opt-ins", "capital ranking") to each variant; measure files touched, lines changed,
   examples needed, regressions on the frozen sets.

## 6. Gates

| Gate | Condition to proceed | If not met |
|---|---|---|
| G-A | A reaches ≥ 99 % accepted precision on H-dev with coverage reported | Fix rules before any classifier work (precision first) |
| G-B (introduce a classifier) | Question 2 above is true **and** B or D adds ≥ 5 pp coverage on H-blind without lowering accepted precision, for < 2 MB and < 0.5 ms p95 | Ship A alone |
| G-C (try a neural model) | Best of A/B/D misses NFR-07 or NFR-08 **and** error analysis shows compositional/word-order failures a linear model cannot represent | Stop at B/D |
| G-final | 15-DECISIONS.md records the choice with all measurements | — |

## 7. Provisional recommendation [HYPOTHESIS]

Build A first (variant A2, lattice), because the hybrid needs A's entity and role layer
anyway and A alone may already meet the targets. In parallel, prepare B-LR on
delexicalized input (cheapest learned option, well calibrated, tiny), so that D is a
measured option rather than a promise. Keep C time-boxed (one week at most, budgets 10k
and 50k parameters) and run it only if G-C triggers, so the decision record can say
whether neural helped instead of assuming it.
