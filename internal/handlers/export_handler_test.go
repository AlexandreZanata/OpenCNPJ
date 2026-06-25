package handlers

import (
	"testing"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

func TestNormalizeCSVExportRequest(t *testing.T) {
	req := models.ExportRequest{Filters: models.SearchFilters{Limit: 0}}
	normalizeCSVExportRequest(&req)
	if req.Filters.Limit != repository.DefaultExportLimit {
		t.Fatalf("default = %d", req.Filters.Limit)
	}

	req.Filters.Limit = 600_000
	normalizeCSVExportRequest(&req)
	if req.Filters.Limit != repository.MaxCSVExportLimit {
		t.Fatalf("capped = %d", req.Filters.Limit)
	}
}
