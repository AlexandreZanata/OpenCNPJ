# 07 — Confidence, ambiguity and semantic safety

Accepted precision matters more than coverage (NFR-07 before NFR-08). The assistant is a
**selective** system: for each utterance it either executes, asks one question, or
declines.

## 1. Decision

```text
Decision = Execute | ExecuteWithNote(note) | Clarify(question, options ≤ 4) | Reject(reason, suggestions)
```

* `ExecuteWithNote`: a default was applied that the user should see ("considerei
  estabelecimentos ativos", "‘recentemente’ = últimos 90 dias").
* `Clarify`: every option is a complete, valid IR (clicking it runs it); free-text answers
  are parsed as `AnswerClarification` (06-CONTEXT.md).
* `Reject`: out of domain, impossible with the data, or blocked by policy; always with 2–3
  supported example questions close to what was asked.

## 2. Signals

| Signal | Computed from | Notes |
|---|---|---|
| `intent_margin` | rules: vote of top intent − second; classifier: calibrated p(top) − p(second) | Main ambiguity signal |
| `coverage` | share of content tokens explained by entities or constructions | Unexplained nouns/verbs ("faturamento", "lucro", "dono") lower it sharply |
| `entity_conf` | min over entities: exact 1.0; phonetic/fuzzy by edit cost and candidate margin; homonyms unresolved → 0 | One weak entity is enough to ask |
| `domain_evidence` | count of domain symbols (activity, unit, place, metric, CNPJ) | **Hard gate:** zero domain symbols ⇒ never execute, whatever a classifier says |
| `known_feature_mass` (classifier only) | fraction of the utterance's hashed features seen in training | Linear models extrapolate confidently on unseen features; low mass ⇒ abstain |
| `ood_score` | explicit out-of-domain class (classifier) and/or a character n-gram domain model score (rules) | Trained/evaluated on the OOD sets (10-SYNTHETIC-DATA.md §6) |
| `validation` | validator issues: Missing / Ambiguous / Impossible | Missing/Ambiguous ⇒ Clarify; Impossible ⇒ Reject with alternatives |

## 3. Policy

A small, ordered policy table (data in `lang/pt-BR/policy.toml`, thresholds tuned on the
validation set only):

1. `domain_evidence = 0` or `ood_score > τ_ood` → **Reject** (out of domain).
2. Blocked intent (people search, CPF, partners, "quem é o dono") → **Reject** (policy, §6).
3. Validation `Impossible` → **Reject** with the nearest supported alternative (e.g.
   "não tenho a situação de 2025; posso mostrar quantas foram registradas em 2025").
4. Validation `Ambiguous` or `entity_conf < τ_ent` → **Clarify** (homonym, fuzzy guess,
   "grandes").
5. `intent_margin < τ_margin` → **Clarify** between the two intents, phrased as the two
   readings ("Você quer o total ou a lista?").
6. `coverage < τ_cov` → **Clarify** ("Não entendi ‘X’. Você quer …?") if a reading exists,
   else **Reject**.
7. Validation `Missing` → **Clarify** for the missing slot, or apply a documented default
   and **ExecuteWithNote**.
8. Otherwise **Execute**.

### Threshold tuning (selective classification)

On the validation sets, sweep each threshold and plot the **risk–coverage curve**
(accepted-error rate vs share answered). Pick the operating point that maximizes coverage
subject to accepted error ≤ 1 % (design goal 0.5 %), then confirm once on the blind set.
This is Chow's reject option and its modern form for classifiers (references §8).

### Calibration

* Classifier scores are calibrated on validation data (temperature scaling for softmax
  outputs; isotonic regression as an alternative) and reported with reliability diagrams
  and expected calibration error.
* Rule votes are not probabilities: map `intent_margin` to empirical precision by isotonic
  regression on validation buckets, so rules and classifiers share one confidence scale
  and one policy.

## 4. Ambiguities handled explicitly

| Input | Why ambiguous | Clarification |
|---|---|---|
| "empresas grandes" | No RFB "grande" porte (SR-08) | 1) porte Demais 2) capital social acima de R$ X (user types X) |
| "Bom Jesus", "São Domingos", "Santa Luzia" | Homonyms across UFs (241 names) | List UFs (≤ 4, otherwise ask for the UF) |
| "sorrizo" with candidates Sorriso/MT and others within distance | Fuzzy | "Você quis dizer Sorriso (MT)?" yes/no |
| "tecnologia" | Sector catalogue maps to several sets (IT services vs electronics manufacturing) | Options with their CNAE divisions |
| "recentemente" | No period | Default last 90 days with note (ExecuteWithNote) |
| "quantas empresas tem" (no place) | Missing place | National count with note (D-08), or ask if the owner prefers |
| "quantas abriram" with no period | Missing period | Default: current year to the data anchor, with note |
| "crescendo" | Metric choice (SR-05) | Default stock change, with note naming the metric |

## 5. Negation and contradictions

* `NEG`/`EXCEPT` attach to the nearest following entity or modifier within the clause
  (constructions `role.except`, 05-INTERPRETER.md §7). Supported: places, sectors, porte,
  MEI, Simples, matriz/filial, status ("que não estão ativas" → status ∈ {03, 04, 08}, with
  a note listing them).
* A negation that attaches to nothing (or to a verb the grammar does not model) ⇒
  **Clarify**; it is never dropped (SR-13). Tests include "não quero padarias, quero
  farmácias" (→ correction act) and "quantas padarias não fecharam" (→ clarify: active
  now, or registered and not baixadas).
* Contradictions detected by the validator (`mei = Yes` with porte Demais; "ativas" with
  status baixada; a municipality outside the UF filter) ⇒ **Reject** with the conflicting
  pair named, or Clarify when one side came from context inheritance.

## 6. Data-protection and misuse guards

* No person-centred intents: names of partners, CPF lookups, "quem é o dono", "telefone do
  fulano" are rejected with a pointer to the regular API, which has its own controls
  (DATA_POLICY.md). The chat never exposes partners.
* Lists respect `--hide-socios`, `--hide-contacts`, plan page size and export rights; CPFs
  embedded in MEI/EI names are masked in chat output by default (D-05).
* Input limits: 500 characters, 64 tokens; all automata are linear-time; fuzzy expansion
  is bounded; one utterance cannot trigger more than one engine query except explicit
  multi-member comparisons (≤ 4).
* Answers never assert causality, opportunity or financial facts (SR-09); the caveat
  catalogue (08-RESPONSES.md §4) is attached by IR features, not by wording.

## 7. What gets logged

Per turn: canonical IR (or none), decision, signals, stage timings, backend, cache hit,
error class. Raw text only for `Clarify`/`Reject` turns and only while `search_log` is on
(D-07), with the same retention and deletion as the existing search log.

## 8. References

* C. K. Chow, "On optimum recognition error and reject tradeoff", IEEE Trans. Inf. Theory
  16(1):41–46, 1970. <https://doi.org/10.1109/TIT.1970.1054406>
* Y. Geifman, R. El-Yaniv, "Selective Classification for Deep Neural Networks", NIPS 2017.
  <https://arxiv.org/abs/1705.08500>
* C. Guo, G. Pleiss, Y. Sun, K. Q. Weinberger, "On Calibration of Modern Neural Networks",
  ICML 2017. <https://arxiv.org/abs/1706.04599>
* D. Hendrycks, K. Gimpel, "A Baseline for Detecting Misclassified and Out-of-Distribution
  Examples in Neural Networks", ICLR 2017. <https://arxiv.org/abs/1610.02136>
