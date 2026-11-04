# 11 — Optional micro classifier

**Conditional.** Built for experiments B, C and D; shipped only if gate G-B passes
(03-RESEARCH-AND-OPTIONS.md §6). The rule interpreter must keep working with the
classifier disabled (`--intent-model off`), which is also the rollback.

## 1. Task

The classifier never sees entity names. Input = the delexicalized sequence produced by
stages S1–S6 of the interpreter (05-INTERPRETER.md), e.g.
`quantas <SECTOR> tem em <MUNI> <UF> ?`, plus the raw characters of non-entity tokens.

Outputs (multitask, one linear head each):

| Head | Classes |
|---|---|
| `act` | new_query, replace_slot, add_filter, remove_filter, compare_with_focus, refer_to_result, correction, answer_clarification, meta, smalltalk, **ood** |
| `intent` | count, list, lookup, compare, rank, breakdown, trend, meta |
| `metric` | stock, registrations, closures, net_registrations, active_change, per10k, inhabitants_per, share, none |
| `direction` | asc, desc, none |

Slot *roles* stay with the constructions; if error analysis shows roles are the main
error source, a small tagger (CRF or averaged perceptron over BIO tags) is the next step
(option E3), not a bigger classifier.

## 2. Features (hashed, no stored vocabulary)

* word unigrams and bigrams of the delexicalized sequence (entity symbols as words);
* symbol trigrams;
* character 3–5-grams of non-entity tokens, with word-boundary markers (robust to typos in
  cue words: "qtas", "quantaz");
* first-token and last-token markers; length bucket;
* dialogue features: `pending_clarification`, `has_last_query`, slot types present in the
  last query (for the `act` head).

Hashing: `xxh3_64(seed, feature)` (already a workspace dependency) into 2^16 or 2^18
buckets (both measured), signed hashing to reduce collision bias.

## 3. Models to compare (experiment B)

| Variant | Training | Notes |
|---|---|---|
| B-NB | counts, Laplace smoothing | Baseline |
| B-LR | multinomial logistic regression, L2 (optionally elastic net for sparsity), AdaGrad SGD, early stopping on S-val | Primary candidate |
| B-SVM | linear SVM one-vs-rest (hinge, SGD) | Scores need Platt/isotonic calibration |
| B-PERC | averaged perceptron | Cheapest training |
| B-FT | fastText supervised CLI (MIT licence, dev only) with `-wordNgrams 2 -minn 3 -maxn 5`, then `quantize` | External baseline; not a production dependency |
| E1-kNN | cosine over hashed char n-gram vectors to exemplars | Distance doubles as OOD score |

Training runs in Rust (`crates/opencnpj-lab`, module `train`) so reproduction needs no
Python. Optional cross-checks with scikit-learn live in `research/python/` (pinned
`requirements.txt`, run with `python3 -I`), never required by the product.

Determinism: fixed seed, single-threaded or order-independent reduction, fixed epoch
count after early stopping is chosen; the manifest records dataset id, hyperparameters,
code commit and the resulting artifact SHA-256. Retraining with the same manifest must
produce the identical artifact.

## 4. Calibration and OOD

* Explicit `ood` class in the `act` head, trained with OOD negatives (10-SYNTHETIC-DATA.md §6).
* Temperature scaling fitted on S-val + H-dev; reliability diagram and ECE reported.
* `known_feature_mass` gate (07-CONFIDENCE-AND-SAFETY.md §2): the share of active features
  whose bucket received non-negligible weight in training.

## 5. Compact artifact

```text
intent-<version>.ocnlu
  header   magic "OCNLU\0v1", format version, flags
  config   hash seed, bucket count, n-gram orders, feature families, label tables per head
  weights  per head: int8 matrix (dense [buckets × classes] or CSR of non-zero rows),
           per-class scale f32, bias f32
  calib    temperature per head
  manifest dataset id, training commit, hyperparameters, metrics on S-val/H-dev
  footer   xxh3 of everything above, length
```

Size [ESTIMATE]: dense 2^16 buckets × 36 classes (all heads) × 1 byte = 2.4 MB; with
elastic-net sparsity of 10–30 % non-zero rows in CSR, 0.3–0.8 MB. Within NFR-05 either way.

Inference: hash ~100 features, add their int8 rows into `i32` accumulators per head (no
allocation; fixed-size arrays), dequantize, apply temperature, softmax. ~100 × 36 adds →
a few microseconds on one slow core [ESTIMATE]. Loaded with `include_bytes!` or mmap and
shared as `Arc`.

Equivalence test: on S-val, Rust int8 inference vs the float model from training must
agree on argmax for ≥ 99.9 % of examples, with max probability difference < 0.01.

## 6. Neural option (experiment C, only through gate G-C)

Character-CNN over the delexicalized string (entity symbols as reserved characters):

| Budget | Shape (example) | Parameters | MACs per query |
|---|---|---|---|
| ~10k | char emb 48×16, conv k=3/4/5 × 24 filters, heads | ~10k | ~0.3 M |
| ~35k | char emb 64×32, conv k=3/4/5 × 64, heads | ~35k | ~1.6 M |
| ~100k | + second conv layer or symbol-level GRU (h=64) | ~100k | ~4 M |

Larger budgets (500k, 1M) only if the learning curve is still rising at 100k. int8
weights; inference hand-written in Rust (convolution + max-pool + linear heads; no
framework). Training offline with a pinned CPU-only toolchain (PyTorch is acceptable
*offline only*, with deterministic flags), exported to the same `.ocnlu` container with a
`model = charcnn` config. Expected 0.1–2 ms per query on one slow core [ESTIMATE] —
within budget, but 10–100× the linear model, so it must earn its place in coverage.

## 7. Rollback and versioning

The language pack manifest pins the classifier SHA-256. Switching model = config change
and reload (same mechanism as snapshot switching). Keep the previous two artifacts. A
classifier that fails its equivalence test or its regression gate on H-dev is never
loaded.
