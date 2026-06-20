# New Project Checklist — BUSCA-CNPJ-2026

> Harness checklist mapped to this repo. Items already satisfied are marked [x].

## Architecture and domain

- [x] **Layers defined** — `docs/ARCHITECTURE.md`, `internal/` packages
- [x] **Entities and aggregates** — `Empresa`, `Estabelecimento`, `Socio`, `Simples`
- [x] **Value Objects** — CNPJ fields, `model.Date` — document invariants as you extend
- [ ] **Business rules** — formal GIVEN/WHEN/THEN for search/export limits
- [ ] **State machines** — import job states if adding async jobs
- [ ] **Access roles** — public read API today; document if auth added
- [ ] **Domain events** — catalog if event-driven import added
- [ ] **Use cases** — add under `docs/use-cases/` per template
- [x] **API contract** — `README.md` + handlers; extend `docs/API-CONTRACT.md` if created
- [x] **Glossary** — `docs/GLOSSARY.md`

## Security

- [x] **Project security** — `docs/SECURITY.md`, gosec, govulncheck in CI
- [x] **OWASP harness rules** — `agent-rules/03-security/`
- [ ] **Agentic ASI** — load if adding LLM features

## Agent harness

- [x] **Harness installed** — `.agent-harness/`, `agent-rules/`, `agent-harness/`
- [x] **AGENTS.md** — project entry point

## Before new features

If any unchecked item applies to your change, **ask before implementing**.
