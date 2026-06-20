# Domain Glossary — BUSCA-CNPJ-2026

> Ubiquitous language for Receita Federal CNPJ data. Code and agents MUST use these terms exactly.

---

## Empresa

**Definition:** Legal entity (company) in Receita Federal base — one record per CNPJ root (8 digits).
**Code name:** `Empresa` (`internal/models/empresa.go`, `internal/model/entities.go`)
**Not the same as:** `Estabelecimento` (branch/unit)

---

## Estabelecimento

**Definition:** Branch or establishment linked to an `Empresa` — full 14-digit CNPJ (root + order + check digits).
**Code name:** `Estabelecimento`, `EstabelecimentoCompleto`
**API path prefix:** `/api/v1/estabelecimentos`

---

## CNPJ

**Definition:** Brazilian company registry identifier — 14 digits for establishment (`cnpj_completo`), 8 for root.
**Validation:** Check digits must be valid; never expose partial IDs without authorization context.
**Code fields:** `cnpj_basico`, `cnpj_ordem`, `cnpj_dv`, `cnpj_completo`

---

## Socio

**Definition:** Partner/shareholder linked to an `Empresa`.
**Code name:** `Socio`

---

## Simples

**Definition:** Simples Nacional tax regime opt-in data for an entity.
**Code name:** `Simples`

---

## CNAE

**Definition:** National classification of economic activity — primary activity on establishment.
**Code field:** `cnae_fiscal_principal`, filter key in search/export

---

## SearchFilters

**Definition:** API input DTO for empresa/estabelecimento search and export.
**Code name:** `SearchFilters` (`internal/models/dto.go`)
**Rule:** Allow-list fields only — no mass assignment from raw JSON

---

## Reference tables

| Term | Code name |
|------|-----------|
| Municipality | `Municipio` |
| Legal nature | `Natureza` |
| Country | `Pais` |
| Qualification | `Qualificacao` |
| Status reason | `Motivo` |

---

## API version

**Prefix:** `/api/v1/` — breaking changes require `/api/v2/`.
