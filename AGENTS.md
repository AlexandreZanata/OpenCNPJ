# AGENTS.md — BUSCA-CNPJ-2026

> Go API for Receita Federal open data (CNPJ search, import, export).
> **Read this first** in every agent session.

**Language:** 100% English for new code, comments, commits, and agent output. Legacy docs in `docs/` may be Portuguese — do not mix languages in new artifacts.

---

## Project + harness layout

| Path | Purpose |
|------|---------|
| `cmd/` | API and importer entrypoints |
| `internal/` | handlers, services, repository, importer pipeline |
| `docs/` | Architecture, security, ADRs (human docs) |
| `.cursor/rules/` | Cursor rules (project + harness) |
| `agent-rules/` | Agent Harness rule library (symlink → `.agent-harness/rules`) |
| `agent-harness/` | Resolve/install scripts (symlink → `.agent-harness/harness`) |
| `.agent-harness/` | Git submodule — [GoodPraticesForLLMSandAgents](https://github.com/AlexandreZanata/GoodPraticesForLLMSandAgents) |

---

## Rules path

```bash
pip install -r agent-harness/requirements.txt   # once
./agent-harness/rules-path.sh                   # → .../agent-rules
```

---

## Always load

1. `agent-rules/AGENT-CORE-PRINCIPLES.md`
2. `agent-rules/09-ai-agent-specific/token-economy.md`
3. `agent-rules/09-ai-agent-specific/anti-hallucination.md`
4. Project Cursor rules: `workflow.mdc`, `code-quality.mdc`, `testing-and-dvt.mdc`, `api-query-budget.mdc`

---

## Conditional load (by task)

See **`.cursor/rules/daily-harness-usage.mdc`** for full daily workflow.

```bash
cd /data/dev/projects/webstorm/BUSCA-CNPJ-2026
./agent-harness/resolve-rules.sh <keywords>
./agent-harness/generate-task-rules.sh api export   # optional
./agent-harness/generate-task-rules.sh --clean       # when task done
```

| Task | Keywords |
|------|----------|
| API / handlers | `api endpoint rest validation` |
| Security | `owasp security authz injection` |
| Import pipeline | `performance query async` |
| Database | `migration query` |
| Bug fix | `bugfix regression error` |

**Domain glossary:** `docs/GLOSSARY.md`  
**Security (Go):** `docs/SECURITY.md` + `agent-rules/03-security/`  
**Architecture:** `docs/ARCHITECTURE.md`

---

## Project delivery gate (mandatory)

From `.cursor/rules/workflow.mdc`:

- `go test ./... -short` (or targeted packages)
- `go vet ./...`
- Propose commit message only — **do not** run `git commit` unless user asks
- Format: `type(scope): subject` max 72 chars — see `docs/COMMIT_CONVENTION.md`

---

## Harness updates

```bash
git submodule update --remote .agent-harness
```

Full harness docs: `.agent-harness/harness/README.md`
