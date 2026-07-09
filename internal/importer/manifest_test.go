package importer

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDiscoverDatasetFindsCoreFiles(t *testing.T) {
	dir := t.TempDir()
	names := []string{
		"K3241.K03200Y0.D60509.EMPRECSV",
		"K3241.K03200Y0.D60509.ESTABELE",
		"K3241.K03200Y0.D60509.SOCIOCSV",
		"F.K03200$W.SIMPLES.CSV.D60509",
		"F.K03200$Z.D60509.CNAECSV",
	}
	for _, name := range names {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("x"), 0o600); err != nil {
			t.Fatal(err)
		}
	}

	ds, err := DiscoverDataset(dir)
	if err != nil {
		t.Fatalf("DiscoverDataset: %v", err)
	}
	if len(ds.Empresas) != 1 || len(ds.Estabelecimentos) != 1 || len(ds.Socios) != 1 {
		t.Fatalf("unexpected dataset: %+v", ds)
	}
	if ds.Simples == "" || ds.CNAEs == "" {
		t.Fatalf("missing simples or cnaes: %+v", ds)
	}
}

func TestDiscoverReferencesFindsLookupFiles(t *testing.T) {
	dir := t.TempDir()
	names := []string{
		"F.K03200$Z.D60509.NATJUCSV",
		"F.K03200$Z.D60509.QUALSCSV",
	}
	for _, name := range names {
		if err := os.WriteFile(filepath.Join(dir, name), []byte("x"), 0o600); err != nil {
			t.Fatal(err)
		}
	}

	ds, err := DiscoverReferences(dir)
	if err != nil {
		t.Fatalf("DiscoverReferences: %v", err)
	}
	if ds.Naturezas == "" || ds.Qualificacoes == "" {
		t.Fatalf("unexpected refs dataset: %+v", ds)
	}
}

func TestRowLimitFullSample(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "sample.csv")
	content := "a\nb\nc\n"
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}
	limit, err := RowLimit(path, 100)
	if err != nil || limit != 0 {
		t.Fatalf("RowLimit(100) = (%d, %v), want (0, nil)", limit, err)
	}
	limit, err = RowLimit(path, 50)
	if err != nil {
		t.Fatal(err)
	}
	if limit < 1 {
		t.Fatalf("expected positive limit, got %d", limit)
	}
}

func TestEstimateLines(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "lines.csv")
	if err := os.WriteFile(path, []byte("1\n2\n3\n4\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	lines, err := estimateLines(path)
	if err != nil {
		t.Fatal(err)
	}
	if lines != 4 {
		t.Fatalf("estimateLines = %d, want 4", lines)
	}
}
