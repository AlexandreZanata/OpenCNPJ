# API Receita Federal - Busca CNPJ

API de alta performance em Go para processamento, indexação e consulta de dados abertos da Receita Federal do Brasil.

## Características

- **Alta Performance**: Consultas < 100ms, throughput de 5000+ req/s
- **Escalável**: Particionamento de tabelas, worker pools, cache Redis
- **Importação Otimizada**: Processamento paralelo com COPY do PostgreSQL
- **API RESTful**: Endpoints para busca, exportação e estatísticas
- **Monitoramento**: Métricas Prometheus, logging estruturado, profiling pprof

## Requisitos

- Go 1.21+
- Docker e Docker Compose
- PostgreSQL 18.4+ (Docker: `postgres:18.4-alpine`)
- Redis 7+
- ClickHouse (opcional)

## Instalação

1. Clone o repositório:
```bash
git clone <repository-url>
cd BUSCA-CNPJ-2026
```

2. Configure as variáveis de ambiente:
```bash
cp .env.example .env
# Edite .env com suas configurações
```

3. Inicie os serviços com Docker Compose:
```bash
docker-compose up -d
```

4. Execute as migrations:
```bash
go run cmd/importer/main.go --skip-refs --skip-empresas --skip-estabelecimentos --skip-socios --skip-simples
# Ou use: migrate -path migrations -database "postgres://..." up
```

5. Importe os dados:
```bash
go run cmd/importer/main.go --data-path=./data --workers=16 --batch-size=10000
```

6. Inicie a API:
```bash
go run cmd/api/main.go
```

## Uso

### Buscar Empresas
```bash
GET /api/v1/empresas/search?razao_social=EMPRESA&limit=10&offset=0
```

### Buscar Estabelecimentos
```bash
GET /api/v1/estabelecimentos/search?cnae=0111301&uf=SP&limit=10
```

### Buscar por CNPJ
```bash
GET /api/v1/estabelecimentos/12345678000190
```

### Exportar CSV
```bash
POST /api/v1/export/csv
Content-Type: application/json

{
  "filters": {
    "cnae_principal": "0111301",
    "uf": "SP",
    "limit": 10000
  },
  "selected_columns": ["cnpj_completo", "nome_fantasia", "razao_social", "cnae_fiscal_principal", "uf"]
}
```

### Estatísticas
```bash
GET /api/v1/stats/cnae?limit=10
GET /api/v1/stats/uf
GET /api/v1/stats/cnae/0111301/uf?limit=10
```

## Estrutura do Projeto

```
.
├── cmd/
│   ├── api/          # Aplicação principal da API
│   └── importer/      # CLI para importação
├── internal/
│   ├── config/        # Configurações
│   ├── database/      # Conexões DB
│   ├── models/         # Modelos de dados
│   ├── repository/     # Camada de acesso a dados
│   ├── handlers/       # Handlers HTTP
│   ├── services/       # Lógica de negócio
│   ├── importer/       # Lógica de importação
│   └── middleware/     # Middlewares
├── migrations/         # SQL migrations
└── data/              # Arquivos CSV de dados
```

## Performance

- **Importação**: < 4h para 150M registros
- **Query CNPJ único**: < 10ms
- **Query com filtros**: < 100ms
- **Exportação 100k**: < 30s
- **Throughput**: 5000+ req/s

## Monitoramento

- Métricas Prometheus: `GET /metrics`
- Health check: `GET /health`
- Profiling (dev): `GET /debug/pprof/*`

## Agent Harness (AI-assisted development)

Open this project in **Cursor** — rules in `.cursor/rules/` load automatically.

See **[.cursor/README.md](.cursor/README.md)** for daily commands.

```bash
cd /data/dev/projects/webstorm/BUSCA-CNPJ-2026
pip install -r agent-harness/requirements.txt

./agent-harness/resolve-rules.sh api performance security

./agent-harness/generate-task-rules.sh api export
./agent-harness/generate-task-rules.sh --clean   # when done
```

- **Agents:** read [AGENTS.md](AGENTS.md) first
- **Domain glossary:** [docs/GLOSSARY.md](docs/GLOSSARY.md)
- **Rules:** `agent-rules/` (symlink to `.agent-harness/rules`)

Update harness: `git submodule update --remote .agent-harness`

## Licença

MIT
