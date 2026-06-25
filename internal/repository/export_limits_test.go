package repository

import "testing"

func TestNormalizeExportLimit(t *testing.T) {
	if got := NormalizeExportLimit(0); got != DefaultExportLimit {
		t.Fatalf("default = %d, want %d", got, DefaultExportLimit)
	}
	if got := NormalizeExportLimit(5000); got != 5000 {
		t.Fatalf("explicit = %d", got)
	}
	if got := NormalizeExportLimit(500000); got != MaxCSVExportLimit {
		t.Fatalf("max = %d, want %d", got, MaxCSVExportLimit)
	}
	if got := NormalizeExportLimit(999999); got != MaxCSVExportLimit {
		t.Fatalf("over max = %d", got)
	}
}
