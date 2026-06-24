package handlers

import "testing"

func TestParseSearchFiltersRejectsOffsetWithCursor(t *testing.T) {
	_, _, err := parseSearchFilters("cnpj:123", 10)
	if err != errOffsetWithCursor {
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
