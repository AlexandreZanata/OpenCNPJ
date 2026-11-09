# Probes 01 — dataset card

Questions that once received a **silently wrong** answer: the engine executed and the answer
looked plausible, but part of the question was ignored or inverted. Each item names its
defect class (`meta.category`) and the date it was found; see
`docs/intelligence/EVALUATION-TASKS.md` (defect classes, ET-09).

| Property | Value |
|---|---|
| Items | 28 (`manifest.json`) |
| Generator | `author_probes.py`, gold built with the skills-export helpers, never from the interpreter |
| Scores | `main` at 16395a9: 4/28; after the fixes of 2026-11-09: 28/28 |
| Human validation | Not performed |

Rules: a probe is never deleted to make a run pass; it is retired only with a documented
reason. New silent errors are added here (or to a new `probes-NN`) with the fix that closes
them, and the probe must fail before the fix.
