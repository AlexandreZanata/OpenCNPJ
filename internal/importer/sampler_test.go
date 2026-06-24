package importer

import (
	"fmt"
	"testing"
)

func TestInSample_10Percent(t *testing.T) {
	var hits int
	const total = 10_000
	for i := 0; i < total; i++ {
		cnpj := formatCNPJ(i)
		if InSample(cnpj, 10) {
			hits++
		}
	}
	ratio := float64(hits) / float64(total)
	if ratio < 0.08 || ratio > 0.12 {
		t.Fatalf("expected ~10%% sample, got %.2f%% (%d/%d)", ratio*100, hits, total)
	}
}

func TestInSample_100Percent(t *testing.T) {
	if !InSample("12345678", 100) {
		t.Fatal("expected full sample")
	}
}

func formatCNPJ(n int) string {
	return fmt.Sprintf("%08d", n)
}
