# Architecture

```text
[Arquivo CSV no disco]
        |
        v
[Reader goroutine] --> chan []string (buffer 10k)
        |
        v
[Parser goroutine pool (N=CPU)] --> chan Model (buffer 10k)
        |
        v
[Batcher] --> acumula N=5000 registros
        |
        v
[COPY goroutine pool (M=DB_CONNS)] --> PostgreSQL 18.4
        |
        v
[Metrics collector] --> rows/s, MB/s, erros, tempo total
```

## Decisions

- Reader CSV com `bufio.NewReaderSize` de 4 MB.
- Conversao ISO-8859-1 para UTF-8 no reader.
- Backpressure com canais bufferizados de 10.000.
- Escrita principal com `COPY FROM` via `pgx/v5`.
