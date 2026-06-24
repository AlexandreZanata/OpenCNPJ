package services

import (
	"testing"

	"busca-cnpj-2026/internal/repository"
)

func TestBuildSearchResponseWithCursor(t *testing.T) {
	cursor := "cnpj:12345678"
	resp := buildSearchResponse([]string{"x"}, repository.PageMeta{
		Total:      10,
		HasMore:    true,
		NextCursor: cursor,
	}, 5, 0)

	if !resp.HasMore || resp.NextCursor == nil || *resp.NextCursor != cursor {
		t.Fatalf("resp = %#v", resp)
	}
}
