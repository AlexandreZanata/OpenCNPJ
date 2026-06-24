# Cursor — BUSCA-CNPJ-2026

Rules in `.cursor/rules/` load automatically when you open this project in Cursor.

## Quick start

1. Open `/data/dev/projects/webstorm/BUSCA-CNPJ-2026` in Cursor.
2. Read [AGENTS.md](../AGENTS.md) for full agent entry point.
3. Follow [rules/daily-harness-usage.mdc](rules/daily-harness-usage.mdc) for every task.

## Daily commands

```bash
cd /data/dev/projects/webstorm/BUSCA-CNPJ-2026
pip install -r agent-harness/requirements.txt   # once

./agent-harness/resolve-rules.sh api performance security

./agent-harness/generate-task-rules.sh api export
./agent-harness/generate-task-rules.sh --clean   # when done
```

## Rule files

| File | Scope |
|------|--------|
| `daily-harness-usage.mdc` | Harness workflow (always on) |
| `busca-cnpj-harness.mdc` | Go project + harness integration |
| `workflow.mdc` | Commits, tests, delivery gate |
| `commit-convention.mdc` | Commit types/scopes (CI commitlint) |
| `code-quality.mdc` | Go code standards |
| `testing-and-dvt.mdc` | Tests and DVT |
| `api-query-budget.mdc` | API query limits |
| `english-only.mdc` | English-only agent output |
| `agent-core-principles.mdc` | Architecture contract |
| `context-discipline.mdc` | Conditional rule loading |
| `token-economy.mdc` | Load minimal rules |
| `_task-active.mdc` | Generated per task (gitignored — delete when done) |

## Update harness

```bash
git submodule update --remote .agent-harness
```
