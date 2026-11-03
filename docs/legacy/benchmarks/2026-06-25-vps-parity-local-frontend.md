# VPS parity — local frontend API benchmark

- **Date**: 2026-06-25T15:49:19-04:00
- **Host**: 16 cores, 31Gi RAM
- **API config**: `config/config.vps-parity.yaml` (rate limit ON, L1 ON, `BENCHMARK_MODE` unset)
- **PostgreSQL**: VPS 16 GB profile via `docker-compose.vps-parity.yml`
- **Partitioning**: LIST (uf)
- **Rows (estabelecimentos)**: 71757702
- **GUCs**: shared_buffers=4GB, work_mem=64MB

## Single-request latency (warm cache, ms)

| Web flow | Endpoint | ms |
|----------|----------|-----|
| CNPJ detail | `GET /estabelecimentos/:cnpj` | 0.45 |
| Empresa search | `GET /empresas/search?razao_social=...` | 11821.52 |
| Estabelecimento UF+text | `GET /estabelecimentos/search?uf=SP&nome_fantasia=...` | 18266.83 |
| Lookup typeahead | `GET /lookup/cnae?q=...` | 18.46 |
| Stats dashboard | `GET /stats/uf` | 0.43 |
| Analytics | `GET /analytics/summary` | 8.04 |

## Sustained load (15s @ concurrency=8)

```
==============================================================
 API AUDIT REPORT
==============================================================
API:         http://localhost:8080
Sample CNPJ: 16036242000175 (basico=16036242, uf=AC)

CONSISTENCY: 14 passed, 0 failed
  [PASS] GET /: status=200 33ms
  [PASS] GET /readyz: status=200 7ms
  [PASS] GET /estabelecimentos/:cnpj: returned=16036242000175 1ms
  [PASS] GET /estabelecimentos/search?cnpj: total=1 61879ms
  [PASS] GET /empresas/search?cnpj_basico: total=1 2735ms
  [PASS] empresa-estabelecimento join: match=ELEICAO 2012 ABGAIL DA SILVA LIMA VEREADOR
  [PASS] GET /empresas/search fuzzy: total=11 rows=10 98794ms
  [PASS] GET /estabelecimentos/search filtered: total=17885 uf=AC cnae=9492800 15799ms
  [PASS] GET /estabelecimentos/search nome_fantasia: total=6 58193ms
  [PASS] GET /stats/cnae: top=4781400 count=3683953 69ms
  [PASS] GET /stats/uf: states=28 sum=71757702 db_estab=71757702 21ms
  [PASS] GET /stats/cnae/:cnae/uf: cnae=4781400 rows=5 22ms
  [PASS] POST /export/csv: csv_lines=101 13596ms
  [PASS] DB row counts loaded: {'socios': 27838421, 'simples': 49034553, 'empresas': 68629147, 'estabelecimentos': 71757702}

PERFORMANCE (15s @ concurrency=8)
Route                 RPS     p50 ms     p95 ms           OK
cnpj_lookup        3685.7        2.0        3.4 55285/55285
cnpj_search        3710.0        2.1        3.4 55650/55650
empresa_fuzzy      2607.6        2.9        4.8 39114/39114
estab_filter       2758.8        2.7        4.9 41382/41382
stats_uf           2907.4        2.6        4.6 43611/43611
```

## Reproduce

```bash
./scripts/local_vps_parity_stack.sh        # clean import + VPS PG profile
./scripts/local_vps_parity_api.sh          # terminal 1
make web-dev                               # terminal 2 → http://localhost:5173
./scripts/local_vps_frontend_benchmark.sh http://localhost:8080
```

## Notes

- Import uses base `docker-compose.yml` (fast COPY flags); API tests use VPS production Postgres GUCs.
- Avoid `?uf=SP&limit=N` without text/CNAE — triggers full-partition `COUNT(*)` on large datasets.
