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

func (idx *Indexer) SyncAll(ctx context.Context, batchSize int) error {
	if err := idx.client.EnsureIndex(ctx, IndexEmpresas, "id"); err != nil {
		return fmt.Errorf("empresas index: %w", err)
	}
	if err := idx.client.EnsureIndex(ctx, IndexEstabelecimentos, "id"); err != nil {
		return fmt.Errorf("estabelecimentos index: %w", err)
	}
	if err := idx.syncEmpresas(ctx, batchSize); err != nil {
		return err
	}
	return idx.syncEstabelecimentos(ctx, batchSize)
}

func (idx *Indexer) syncEmpresas(ctx context.Context, batchSize int) error {
	offset := 0
	for {
		rows, err := idx.db.QueryContext(ctx, `
			SELECT cnpj_basico, razao_social
			FROM empresas
			ORDER BY cnpj_basico
			LIMIT $1 OFFSET $2`, batchSize, offset)
		if err != nil {
			return fmt.Errorf("query empresas: %w", err)
		}
		docs, err := scanEmpresaDocs(rows)
		rows.Close()
		if err != nil {
			return err
		}
		if len(docs) == 0 {
			break
		}
		if err := idx.client.AddDocuments(ctx, IndexEmpresas, docs); err != nil {
			return err
		}
		idx.logger.Printf("meilisearch: indexed %d empresas (offset %d)", len(docs), offset)
		offset += batchSize
	}
	return nil
}

func (idx *Indexer) syncEstabelecimentos(ctx context.Context, batchSize int) error {
	offset := 0
	for {
		rows, err := idx.db.QueryContext(ctx, `
			SELECT id::text, cnpj_completo, COALESCE(nome_fantasia, ''), situacao_cadastral
			FROM estabelecimentos
			WHERE situacao_cadastral = '02'
			ORDER BY id
			LIMIT $1 OFFSET $2`, batchSize, offset)
		if err != nil {
			return fmt.Errorf("query estabelecimentos: %w", err)
		}
		docs, err := scanEstabDocs(rows)
		rows.Close()
		if err != nil {
			return err
		}
		if len(docs) == 0 {
			break
		}
		if err := idx.client.AddDocuments(ctx, IndexEstabelecimentos, docs); err != nil {
			return err
		}
		idx.logger.Printf("meilisearch: indexed %d estabelecimentos (offset %d)", len(docs), offset)
		offset += batchSize
	}
	return nil
}

func scanEmpresaDocs(rows *sql.Rows) ([]map[string]interface{}, error) {
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

func scanEstabDocs(rows *sql.Rows) ([]map[string]interface{}, error) {
	docs := make([]map[string]interface{}, 0)
	for rows.Next() {
		var id, cnpj, nome, situacao string
		if err := rows.Scan(&id, &cnpj, &nome, &situacao); err != nil {
			return nil, err
		}
		docs = append(docs, map[string]interface{}{
			"id":                 id,
			"cnpj_completo":      cnpj,
			"nome_fantasia":      nome,
			"situacao_cadastral": situacao,
		})
	}
	return docs, rows.Err()
}
