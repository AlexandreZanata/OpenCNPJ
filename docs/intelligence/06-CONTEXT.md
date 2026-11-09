# 06 — Conversational context without an LLM

The assistant keeps a small structured state per conversation, not the text history.
Each new utterance is interpreted as a **dialogue act** applied to that state.

## 1. State

```rust
pub struct DialogueState {
    pub version: u8,                          // IR_VERSION of the encoder
    pub competence: Competence,               // pinned at the first turn (answers stay consistent)
    pub last: Option<ValidRequest>,           // canonical IR of the last executed query
    pub focus: SmallVec<[EntityRef; 4]>,      // last mentioned places/sectors, most recent first
    pub last_result: Option<ResultRef>,       // e.g. top-k municipality ids of the last ranking ("dessas cidades")
    pub pending: Option<Clarification>,       // the question the assistant asked and its options
    pub turn: u16,
}
```

Size: the canonical IR encodes in 20–80 bytes, focus ≤ 4 × 3 bytes, a ranking reference
≤ 10 ids × 2 bytes, a clarification ≤ 40 bytes → **typically 60–160 bytes, worst case
< 256 bytes** (CR-01) **[ESTIMATE]**; measured in WP-4.6.

Transport: the server returns the state as an opaque `context` token =
`base64url(version ‖ encoded state ‖ HMAC-SHA-256(key, …)[..16])`; the client sends it
back with the next message. The chat node stays stateless (no session memory to grow, no
sticky routing, survives restarts and upgrades). Tampered or stale tokens (bad MAC, other
`IR_VERSION`, other competence no longer served) are rejected with a friendly restart.
A server-side map (`moka`, TTL 30 min) is the fallback for clients that cannot keep the
token. The HMAC key rotates with a key id byte; `hmac` (RustCrypto, same family as the
`sha2` already in the workspace) is the only new dependency.

## 2. Dialogue acts

The interpreter classifies each utterance into one act before building the IR. Acts are
recognized from discourse symbols (05-INTERPRETER.md §4) and from what the utterance
lacks: an utterance with no intent cue and only entities or modifiers is elliptical.

| Act | Signals | Effect on the new IR | Example after "Quantas padarias existem em Sorriso?" |
|---|---|---|---|
| `NewQuery` | full intent cue + own activity/place, no anaphor or follow-up marker | Built from scratch; nothing inherited except competence | "Quantas farmácias tem em Cuiabá?" |
| `ReplaceSlot` | `AND_WHAT` / bare entity of a type present in `last` ("E em Sinop?", "e farmácias?") | Copy `last`, replace the slot of that entity type | "E em Sinop?" → place = Sinop |
| `AddFilter` | `NOW`/`ONLY`/modifier without intent cue ("agora só as matrizes", "apenas MEI") | Copy `last`, add/override that filter | "Agora considera apenas as matrizes" → head_office = Yes |
| `RemoveFilter` | `RESET`/"tira", "sem o filtro de", "todas as situações", "esquece o porte" | Copy `last`, drop the named filter (back to default) | "tira o filtro de matriz" |
| `CompareWithFocus` | comparison/ranking cue without enough members; uses `focus` | `Compare` over focus entities with the new metric | "Qual tem mais por habitante?" → Compare [Sorriso, Sinop] per10k |
| `ReferToResult` | `ANAPHOR` + plural ("dessas cidades", "desses setores", "delas") | Place/sector filter = `last_result` ids | "Me mostra as empresas dessas cidades" |
| `Correction` | `CORRECT` ("na verdade", "quis dizer", "não, Sinop") | Re-run `last` with the corrected slot; previous answer marked superseded | "Não, eu quis dizer Sinop" |
| `AnswerClarification` | `pending` is set and the utterance matches an option (number, option words, `YES`/`NO`, a value of the expected type) | Fill the pending slot, then validate again | Assistant: "Qual Bom Jesus? 1 PI, 2 RS, …" → "o do RS" |
| `MetaQuestion` | "como você calculou", "de quando são os dados", "quais CNAEs" | Static answer about `last` | |
| `SmallTalk` | `THANKS`, greeting, `HELP` | Fixed reply; state unchanged | "valeu" |
| `OutOfDomain` | no domain symbols, low scores | Reject with suggestions; state unchanged | "qual a previsão do tempo?" |

Precedence: `AnswerClarification` (when pending) → `Correction` → `ReferToResult` →
`CompareWithFocus` → `ReplaceSlot`/`AddFilter`/`RemoveFilter` → `NewQuery`. The table is
compiled as constructions with an `act` vote, not as nested conditions.

## 3. Inheritance rules

1. **A new self-contained question inherits nothing** (CR-03), except the pinned
   competence and, for place homonyms only, the UF in focus as a disambiguation prior.
2. **Elliptical utterances inherit everything** they do not override.
3. **Type-directed replacement.** A bare entity replaces the slot of the same type; if
   `last` has no slot of that type, the entity is added as a filter ("e só MEI?").
4. **Metric changes keep the scope** ("e por habitante?" keeps place and sector, changes
   the metric).
5. **Intent changes with a clear new cue keep the scope only when an anaphor/follow-up
   marker is present** ("e quantas fecharam no ano passado?" keeps the sector and place:
   `AND_WHAT` marker + no new activity/place).
6. **Focus update.** After each executed query, `focus` = entities of its place/sector
   slots (compare members included); `last_result` = ids shown in the answer (ranking rows,
   breakdown members).
7. **Clarification lifetime.** A pending clarification lasts one turn; an utterance that is
   clearly a new question discards it.

## 4. Scripted examples (they become the dialogue test suite)

```text
U1 Quantas padarias existem em Sorriso?
   NewQuery   COUNT est WHERE muni=Sorriso/MT sector=bakery status=active
U2 E em Sinop?
   ReplaceSlot(place)   COUNT est WHERE muni=Sinop/MT sector=bakery status=active   focus=[Sinop, Sorriso]
U3 Qual tem mais por habitante?
   CompareWithFocus   COMPARE muni [Sinop, Sorriso] METRIC per10k(stock) WHERE sector=bakery status=active
U4 Agora considera apenas as matrizes.
   AddFilter(head_office=Yes)   same COMPARE + head_office=Yes
U5 E quantas fecharam no ano passado?
   NewIntent with AND_WHAT: COMPARE muni [Sinop, Sorriso] METRIC closures PERIOD 2025 WHERE sector=bakery head_office=Yes
U6 Quantas farmácias tem em Cuiabá?
   NewQuery (full cue, new sector and place) → nothing inherited
```

```text
U1 Quais cidades de MT têm menos farmácias por habitante?
   RANK muni WITHIN uf=MT METRIC per10k(stock) DIR asc K 10 MIN_BASE … sector=pharmacy
U2 Me mostra as farmácias dessas cidades.
   ReferToResult   LIST est WHERE muni ∈ last_result sector=pharmacy status=active
U3 Só as que têm telefone.
   AddFilter(contact=phone)   (List only)
```

```text
U1 Quantas empresas grandes existem em Sorriso?
   Ambiguous(porte "grande") → clarification:
   "O que você considera empresa grande? 1) porte 'Demais' da Receita (não é ME nem EPP) 2) capital social acima de um valor"
U2 1
   AnswerClarification → COUNT est WHERE muni=Sorriso porte={05} status=active
```

## 5. Failure modes and guards

| Risk | Guard |
|---|---|
| Inheriting filters into an unrelated question | Rule 1 + tests with topic switches in the dialogue suite (target: zero leaks) |
| "E em Sinop?" after a ranking (no place slot) | Rule 3: add as place filter, rank becomes within Sinop → validator may reject (ranking municipalities within one municipality) → clarification |
| Anaphor without antecedent ("dessas cidades" on turn 1) | Clarification ("Quais cidades?") |
| Token from an old competence after the monthly switch | Competence stays pinned while served; otherwise restart message |
| Very long conversations | State is bounded; nothing grows with turns |
