# Architecture

## Import pipeline

```text
[CSV file on disk]
        |
        v
[Reader goroutine] --> chan []string (buffer 10k)
        |
        v
[Parser goroutine pool (N=CPU)] --> chan Model (buffer 10k)
        |
        v
[Batcher] --> accumulates N=5000 records
        |
        v
[COPY goroutine pool (M=DB_CONNS)] --> PostgreSQL 18.4
        |
        v
[Metrics collector] --> rows/s, MB/s, errors, total time
```

## API layers

```text
HTTP (Fiber) → handlers → services → repository → PostgreSQL / Redis
```

| Layer | Package | Responsibility |
|-------|---------|----------------|
| Interfaces | `internal/handlers`, `cmd/api` | HTTP routing, validation, response mapping |
| Application | `internal/services` | Use cases, caching, export orchestration |
| Domain | `internal/models`, `internal/model` | Entities, DTOs, value objects |
| Infrastructure | `internal/repository`, `internal/database` | SQL, Redis, connection pools |

## Key decisions

- CSV reader uses `bufio.NewReaderSize` with 4 MB buffer.
- ISO-8859-1 to UTF-8 conversion in the reader.
- Backpressure via buffered channels (10,000 capacity).
- Primary write path: `COPY FROM` via `pgx/v5`.
- Search: `pg_trgm` GIN indexes on `razao_social` and `nome_fantasia`.
- Analytics: pre-aggregated tables (`migrations/000009_*`).

See [ADR/001-use-pg-copy.md](ADR/001-use-pg-copy.md) for bulk-load rationale.

## Deployment topology

```text
[Browser] → [React web] → [Go API :8080]
                              |
                    +---------+---------+
                    |                   |
              [PostgreSQL]          [Redis]
```

Optional: Prometheus scrapes `GET /metrics`.
