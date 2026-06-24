package importer

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPickLatestByPartitionKeepsNewestShard(t *testing.T) {
	dir := t.TempDir()
	names := []string{
		"K3241.K03200Y0.D60509.EMPRECSV",
		"K3241.K03200Y0.D60613.EMPRECSV",
		"K3241.K03200Y1.D60509.EMPRECSV",
		"K3241.K03200Y1.D60613.EMPRECSV",
	}
	paths := make([]string, len(names))
	for i, name := range names {
		path := filepath.Join(dir, name)
		if err := os.WriteFile(path, []byte("x"), 0o600); err != nil {
			t.Fatal(err)
		}
		paths[i] = path
	}

	got := pickLatestByPartition(paths)
	if len(got) != 2 {
		t.Fatalf("pickLatestByPartition = %d files, want 2", len(got))
	}
	for _, path := range got {
		if !strings.Contains(path, "D60613") {
			t.Fatalf("expected D60613 snapshot, got %s", path)
		}
	}
}

func TestDiscoverDatasetPrefersNewestSnapshot(t *testing.T) {
	dir := t.TempDir()
	names := []string{
		"K3241.K03200Y0.D60509.EMPRECSV",
		"K3241.K03200Y0.D60613.EMPRECSV",
		"K3241.K03200Y1.D60509.EMPRECSV",
		"K3241.K03200Y1.D60613.EMPRECSV",
		"F.K03200$W.SIMPLES.CSV.D60509",
		"F.K03200$W.SIMPLES.CSV.D60613",
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
	if len(ds.Empresas) != 2 {
		t.Fatalf("empresas = %d files, want 2: %+v", len(ds.Empresas), ds.Empresas)
	}
	if !strings.Contains(ds.Simples, "D60613") {
		t.Fatalf("simples should be newest snapshot, got %s", ds.Simples)
	}
}
