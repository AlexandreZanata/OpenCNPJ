# Documentation Index

Enterprise documentation for the OpenCNPJ platform.

## Getting started

| Document | Audience | Description |
|----------|----------|-------------|
| [QUICKSTART.md](QUICKSTART.md) | All | **One-command download + import** |
| [README.md](../README.md) | All | Project overview |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contributors | Local setup, PR workflow |
| [COMMIT_CONVENTION.md](COMMIT_CONVENTION.md) | Contributors | Commit message format |
| [AGENTS.md](../AGENTS.md) | AI agents | Harness workflow entry point |

## Architecture and design

| Document | Description |
|----------|-------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Import pipeline and system layers |
| [GLOSSARY.md](GLOSSARY.md) | Domain terms (Empresa, Estabelecimento, CNPJ…) |
| [ADR/001-use-pg-copy.md](ADR/001-use-pg-copy.md) | Decision: PostgreSQL COPY for bulk load |
| [DATA_FORMATS.md](DATA_FORMATS.md) | RFB CSV column mapping and parser rules |
| [HIGH_QUERY_ROUTES.md](HIGH_QUERY_ROUTES.md) | Routes exceeding SQL query budget |

## Operations

| Document | Description |
|----------|-------------|
| [IMPORT.md](IMPORT.md) | Full and sample data import |
| [HARDWARE.md](HARDWARE.md) | RAM/CPU/disk tuning by machine |
| [PERFORMANCE.md](PERFORMANCE.md) | Import tuning and API latency targets |
| [ops/SAAS-VPS-DEPLOY.md](ops/SAAS-VPS-DEPLOY.md) | SaaS API on VPS (comerc.app.br) — templates only |
| [ops/NGINX-SAAS.md](ops/NGINX-SAAS.md) | Nginx + Cloudflare for api/admin subdomains |
| [ops/ADMIN-AUTH.md](ops/ADMIN-AUTH.md) | Admin login, TOTP MFA, JWT, bootstrap CLI |
| [ops/ADMIN-PANEL.md](ops/ADMIN-PANEL.md) | Server-rendered admin UI (no Node on VPS) |
| [ops/DUAL-DATABASE-VPS.md](ops/DUAL-DATABASE-VPS.md) | Two Postgres DBs on VPS (CNPJ + SaaS) |
| [ops/CNAE-PARTITIONING.md](ops/CNAE-PARTITIONING.md) | CNAE HASH sub-partitions under LIST(uf) |
| [api/QUICKSTART.md](api/QUICKSTART.md) | Customer API quickstart (CNPJ lookup) |
| [api/OPENAPI.yaml](api/OPENAPI.yaml) | OpenAPI 3.1 spec (v1 CNPJ route) |
| [benchmarks/README.md](benchmarks/README.md) | Benchmark suite overview |
| [benchmarks/COMPARISON.md](benchmarks/COMPARISON.md) | Approach comparison results |
| [benchmarks/HARDWARE-RTX4060-32GB.md](benchmarks/HARDWARE-RTX4060-32GB.md) | Import speed (32 GB RAM / RTX 4060) |

## Security and quality

| Document | Description |
|----------|-------------|
| [SECURITY.md](SECURITY.md) | Security policy, severity, disclosure |
| [SECURITY-COMMANDS.md](SECURITY-COMMANDS.md) | Local and CI security tooling |
| [DVT.md](DVT.md) | Technical debt / integration test backlog |

## Open source

| Document | Description |
|----------|-------------|
| [OPEN_SOURCE.md](OPEN_SOURCE.md) | OSS principles and scope |
| [LICENSING.md](LICENSING.md) | MIT OR Apache-2.0 dual license |
| [../LICENSE](../LICENSE) | MIT License text |
| [../LICENSE-APACHE-2.0](../LICENSE-APACHE-2.0) | Apache 2.0 License text |
| [../CODE_OF_CONDUCT.md](../CODE_OF_CONDUCT.md) | Community standards |
| [../CHANGELOG.md](../CHANGELOG.md) | Release history |

## Frontend

| Document | Description |
|----------|-------------|
| [../web/README.md](../web/README.md) | React portal setup and routes |
