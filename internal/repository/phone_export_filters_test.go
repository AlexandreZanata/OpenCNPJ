package repository

import (
	"fmt"
	"strings"
	"testing"

	"busca-cnpj-2026/internal/models"
)

func TestParseExportDate(t *testing.T) {
	if _, err := parseExportDate("2020-05-15"); err != nil {
		t.Fatalf("valid date rejected: %v", err)
	}
	if _, err := parseExportDate("15-05-2020"); err == nil {
		t.Fatal("expected invalid format error")
	}
}

func TestBuildPhoneExportQueryWithDateRange(t *testing.T) {
	query, args, err := buildPhoneExportQuery(models.PhoneExportRequest{
		Category:    "advocacia",
		CreatedFrom: "2020-01-01",
		CreatedTo:   "2024-12-31",
		Limit:       100,
	})
	if err != nil {
		t.Fatalf("build query: %v", err)
	}
	if !strings.Contains(query, "data_inicio_atividade >=") {
		t.Fatalf("missing from filter: %s", query)
	}
	if !strings.Contains(query, "data_inicio_atividade <=") {
		t.Fatalf("missing to filter: %s", query)
	}
	if !strings.Contains(query, " LIMIT $") {
		t.Fatalf("missing LIMIT clause: %s", query)
	}
	limit, ok := args[len(args)-1].(int)
	if !ok || limit != 100 {
		t.Fatalf("last arg must be limit int 100, got %#v", args[len(args)-1])
	}
	wantLimit := fmt.Sprintf(" LIMIT $%d", len(args))
	if !strings.Contains(query, wantLimit) {
		t.Fatalf("expected %q in query, got: %s", wantLimit, query)
	}
}

func TestBuildPhoneExportQueryExportAll(t *testing.T) {
	exportAll := true
	query, _, err := buildPhoneExportQuery(models.PhoneExportRequest{
		Category:  "advocacia",
		ExportAll: &exportAll,
	})
	if err != nil {
		t.Fatalf("build query: %v", err)
	}
	if strings.Contains(query, " LIMIT ") {
		t.Fatalf("export_all should omit LIMIT: %s", query)
	}
}

func TestBuildPhoneExportQueryInvalidDate(t *testing.T) {
	_, _, err := buildPhoneExportQuery(models.PhoneExportRequest{
		Category:    "advocacia",
		CreatedFrom: "not-a-date",
	})
	if err == nil {
		t.Fatal("expected invalid date error")
	}
}
