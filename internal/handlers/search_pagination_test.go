package handlers

import (
	"errors"
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestParseSearchFiltersRejectsOffsetWithCursor(t *testing.T) {
	_, _, err := parseSearchFilters("cnpj:123", 10)
	if !errors.Is(err, errOffsetWithCursor) {
		t.Fatalf("err = %v", err)
	}
}

func TestParseSearchFiltersCursorOnly(t *testing.T) {
	cursor, offset, err := parseSearchFilters("cnpj:123", 0)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if cursor != "cnpj:123" || offset != 0 {
		t.Fatalf("cursor=%q offset=%d", cursor, offset)
	}
}

func TestApplyPaginationToResponse(t *testing.T) {
	resp := &models.SearchResponse{}
	applyPaginationToResponse(resp, "cnpj:123")
	if resp.NextCursor == nil || *resp.NextCursor != "cnpj:123" {
		t.Fatalf("next_cursor = %#v", resp.NextCursor)
	}
}
