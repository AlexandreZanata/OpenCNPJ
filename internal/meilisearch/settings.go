package meilisearch

import (
	"context"
	"fmt"
)

// SyncOptions controls indexer batching and selective matriz scope.
type SyncOptions struct {
	BatchSize             int
	MaxBatches            int // 0 = unlimited per stream
	SelectiveActiveMatriz bool
}

// ConfigureSelectiveIndexes applies searchable/filterable attributes for hybrid search.
func (c *Client) ConfigureSelectiveIndexes(ctx context.Context) error {
	if err := c.patchSettings(ctx, IndexEmpresas, map[string]interface{}{
		"searchableAttributes": []string{"razao_social", "cnpj_basico"},
		"filterableAttributes": []string{"cnpj_basico"},
	}); err != nil {
		return fmt.Errorf("empresas settings: %w", err)
	}
	return c.patchSettings(ctx, IndexEstabelecimentos, map[string]interface{}{
		"searchableAttributes": []string{"nome_fantasia", "cnpj_completo"},
		"filterableAttributes": []string{"uf", "situacao_cadastral"},
	})
}

func (c *Client) patchSettings(ctx context.Context, uid string, settings map[string]interface{}) error {
	return c.patch(ctx, fmt.Sprintf("/indexes/%s/settings", uid), settings, nil)
}
