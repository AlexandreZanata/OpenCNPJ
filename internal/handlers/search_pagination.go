package handlers

import (
	"errors"

	"busca-cnpj-2026/internal/models"
)

var errOffsetWithCursor = errors.New("offset cannot be used together with cursor")

func parseSearchFilters(cursor string, offset int) (string, int, error) {
	if cursor == "" {
		return "", offset, nil
	}
	if offset > 0 {
		return "", 0, errOffsetWithCursor
	}
	return cursor, 0, nil
}

// applyPaginationToResponse is kept for handler tests documenting cursor response shape.
func applyPaginationToResponse(resp *models.SearchResponse, nextCursor string) {
	if nextCursor != "" {
		resp.NextCursor = &nextCursor
	}
}
