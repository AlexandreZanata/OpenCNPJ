# Export throughput benchmark (VPS parity local)

- **Date**: 2026-06-25T16:04:18
- **API**: http://localhost:8080
- **Config**: `config/config.vps-parity.yaml` · Postgres VPS profile
- **Dataset**: 71757702 estabelecimentos · partitioning: LIST(uf)

## Changes (this benchmark run)

| Change | Path |
|--------|------|
| CSV export max raised to **500,000** rows/request | `internal/repository/export_limits.go` |
| Export handler applies `NormalizeExportLimit` | `internal/handlers/export_handler.go` |
| Benchmark script + 500k bulk test | `scripts/benchmark_export.sh` |
| VPS parity local stack (import + PG profile) | `scripts/local_vps_parity_stack.sh` |
| Frontend/API parity config | `config/config.vps-parity.yaml` |

## CSV export — filtered CNAE (`uf=SP`, `cnae=4781400`, active)

6 columns: cnpj_completo, nome_fantasia, razao_social, cnae, uf, municipio.

| Limit | Time (ms) | Rows | Size (MB) | Rows/s |
|-------|-----------|------|-----------|--------|
| 1000 | 4096.49 | 1000 | 0.07 | 244.1 |
| 5000 | 4067.15 | 5000 | 0.33 | 1229.4 |
| 10000 | 4945.2 | 10000 | 0.66 | 2022.2 |

## CSV bulk export — 500k (`uf=SP`, `situacao_cadastral=02`, `limit=500000`)

| Limit | Time (ms) | Rows | Size (MB) | Rows/s |
|-------|-----------|------|-----------|--------|
| 500000 | 21873.39 | 500000 | 32.42 | 22858.8 |

## Phone export (`POST /api/v1/export/phones`)

Filter: `category=restaurante`, `uf=SP`, `only_active=true`.

| Limit | Time (ms) | Rows | Size (MB) | Rows/s |
|-------|-----------|------|-----------|--------|
| 1000 | 76947.08 | 1000 | 0.1 | 13.0 |
| 5000 | 2596.69 | 5000 | 0.5 | 1925.5 |
| 10000 | 2717.39 | 10000 | 1.0 | 3680.0 |

## Reproduce

```bash
go test ./internal/repository/... -short -run NormalizeExportLimit
./scripts/benchmark_export.sh http://localhost:8080
```

## Limits

| Endpoint | Default | Max |
|----------|---------|-----|
| `POST /api/v1/export/csv` | 10,000 | **500,000** |
| `POST /api/v1/export/phones` | 5,000 | 50,000 |

## Notes

- Frontend `ExportPanel` still sends `limit: 1000`; raise in UI for larger exports.
- Use UF (+ CNAE when possible) for LIST(uf) partition pruning.
- 500k export (SP active, warm cache): **~22 s**, **~22.9k rows/s**, **32.4 MB** CSV.
- Phone export first request after idle may be slow (cold plan).
