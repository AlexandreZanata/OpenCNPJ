package meilisearch

import (
	"context"
	"database/sql"
	"fmt"
	"log"
)

type Indexer struct {
	client *Client
	db     *sql.DB
	logger *log.Logger
}

func NewIndexer(client *Client, db *sql.DB, logger *log.Logger) *Indexer {
	if logger == nil {
		logger = log.Default()
	}
	return &Indexer{client: client, db: db, logger: logger}
}

func (idx *Indexer) SyncAll(ctx context.Context, opts SyncOptions) error {
	if opts.BatchSize <= 0 {
		opts.BatchSize = 5000
	}
	if err := idx.client.EnsureIndex(ctx, IndexEmpresas, "id"); err != nil {
		return fmt.Errorf("empresas index: %w", err)
	}
	if err := idx.client.EnsureIndex(ctx, IndexEstabelecimentos, "id"); err != nil {
		return fmt.Errorf("estabelecimentos index: %w", err)
	}
	if err := idx.client.ConfigureSelectiveIndexes(ctx); err != nil {
		return fmt.Errorf("index settings: %w", err)
	}
	if err := idx.syncEmpresas(ctx, opts); err != nil {
		return err
	}
	return idx.syncEstabelecimentos(ctx, opts)
}

func (idx *Indexer) syncEmpresas(ctx context.Context, opts SyncOptions) error {
	query := FullEmpresaSQL
	if opts.SelectiveActiveMatriz {
		query = SelectiveEmpresaSQL
		idx.logger.Println("meilisearch: selective empresa index (active matriz)")
	}
	return idx.syncStream(ctx, IndexEmpresas, query, opts, idx.scanEmpresaRows)
}

func (idx *Indexer) syncEstabelecimentos(ctx context.Context, opts SyncOptions) error {
	query := FullEstabSQL
	if opts.SelectiveActiveMatriz {
		query = SelectiveEstabSQL
		idx.logger.Println("meilisearch: selective estabelecimentos index (active matriz)")
	}
	return idx.syncStream(ctx, IndexEstabelecimentos, query, opts, idx.scanEstabRows)
}

type rowScanner func(rows *sql.Rows) ([]map[string]interface{}, error)

func (idx *Indexer) syncStream(
	ctx context.Context,
	indexUID, query string,
	opts SyncOptions,
	scan rowScanner,
) error {
	offset := 0
	batches := 0
	for {
		if opts.MaxBatches > 0 && batches >= opts.MaxBatches {
			idx.logger.Printf("meilisearch: stopped at max_batches=%d for %s", opts.MaxBatches, indexUID)
			return nil
		}
		docs, err := idx.fetchDocs(ctx, query, opts.BatchSize, offset, scan)
		if err != nil {
			return err
		}
		if len(docs) == 0 {
			return nil
		}
		if err := idx.client.AddDocuments(ctx, indexUID, docs); err != nil {
			return err
		}
		idx.logger.Printf("meilisearch: indexed %d %s (offset %d)", len(docs), indexUID, offset)
		offset += opts.BatchSize
		batches++
	}
}

func (idx *Indexer) fetchDocs(
	ctx context.Context,
	query string,
	batchSize, offset int,
	scan rowScanner,
) ([]map[string]interface{}, error) {
	rows, err := idx.db.QueryContext(ctx, query, batchSize, offset)
	if err != nil {
		return nil, fmt.Errorf("query index batch: %w", err)
	}
	defer rows.Close()
	docs, err := scan(rows)
	if err != nil {
		return nil, err
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	return docs, nil
}

func (idx *Indexer) scanEmpresaRows(rows *sql.Rows) ([]map[string]interface{}, error) {
	docs := make([]map[string]interface{}, 0)
	for rows.Next() {
		var basico, razao string
		if err := rows.Scan(&basico, &razao); err != nil {
			return nil, err
		}
		docs = append(docs, map[string]interface{}{
			"id":           basico,
			"cnpj_basico":  basico,
			"razao_social": razao,
		})
	}
	return docs, rows.Err()
}

func (idx *Indexer) scanEstabRows(rows *sql.Rows) ([]map[string]interface{}, error) {
	docs := make([]map[string]interface{}, 0)
	for rows.Next() {
		var id, cnpj, nome, situacao, uf string
		if err := rows.Scan(&id, &cnpj, &nome, &situacao, &uf); err != nil {
			return nil, err
		}
		docs = append(docs, map[string]interface{}{
			"id":                 id,
			"cnpj_completo":      cnpj,
			"nome_fantasia":      nome,
			"situacao_cadastral": situacao,
			"uf":                 uf,
		})
	}
	return docs, rows.Err()
}
