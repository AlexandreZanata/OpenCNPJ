# OpenCNPJ API changelog

Format based on [Keep a Changelog](https://keepachangelog.com/). Versioning follows [SemVer](https://semver.org/) for the public HTTP contract.

## [1.0.0] - 2026-07-07

### Added

- `GET /api/v1/cnpj/{cnpj}` — authenticated CNPJ lookup (14-digit path parameter).
- `X-API-Key` authentication (`ocnpj_live_<32 hex>`).
- Per-client rate limit (default 60 req/min) and optional monthly quota.
- Response fields: `razao_social`, `nome_fantasia`, `cnae_principal`, `endereco`, `socios`, `simples`.
- `GET /readyz` — readiness probe (no auth).

### Documentation

- OpenAPI 3.1 spec: `docs/api/OPENAPI.yaml`
- Quickstart: `docs/api/QUICKSTART.md`
- Error catalog: `docs/api/ERRORS.md`

### Notes

- v1 is **CNPJ lookup only**; search/export routes remain internal OSS API when `saas.public_api_only` is false.

[1.0.0]: https://github.com/AlexandreZanata/BUSCA-CNPJ-2026/releases/tag/api-v1.0.0
