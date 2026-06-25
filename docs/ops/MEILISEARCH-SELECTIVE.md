# Meilisearch selective index (OpenCNPJ plan 02 Phase 5)

Indexes **active matriz** establishments only (`situacao_cadastral = 02`, `identificador_matriz_filial = 1`) plus matching `empresas` rows. Target ~20M documents on full RFB dataset (vs ~150M+ all branches).

## Config

```yaml
# config/config.yaml
meilisearch:
  enabled: true
  selective_active_matriz: true
  host: localhost
  port: 7700
  api_key: <master-key>
```

Postgres remains source of truth; Meilisearch handles **text-only** search (no UF/CNAE filters — those stay on PostgreSQL).

## Index

```bash
docker compose up -d meilisearch
# enable meilisearch.enabled in config
./scripts/meilisearch_selective_index.sh
```

Dev sample (1 batch):

```bash
MEILI_STRICT=1 ./scripts/opencnpj_advanced_phase5.sh http://localhost:8080
# or:
go run ./cmd/meilisearch-index -batch-size 500 -max-batches 2
```

Importer auto-syncs when `meilisearch.enabled: true`.

## VPS memory (16 GB)

Example env: `deploy/vps/meilisearch-opencnpj.env.example`

- `MEILI_MAX_INDEXING_MEMORY=6Gb` during bulk index
- Keep selective mode on to stay within RAM budget

## Verify

```bash
./scripts/opencnpj_advanced_phase5.sh http://localhost:8080
MEILI_STRICT=1 ./scripts/opencnpj_advanced_phase5.sh http://localhost:8080
```

## References

- Meilisearch [index settings](https://www.meilisearch.com/docs/reference/api/settings)
- `internal/meilisearch/selective.go` — SQL filters
- DVT-17 / DVT-24
