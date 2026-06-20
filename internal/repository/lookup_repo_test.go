package repository

import "testing"

func TestNormalizeLookupLimit(t *testing.T) {
	if got := normalizeLookupLimit(0); got != 15 {
		t.Fatalf("default = %d", got)
	}
	if got := normalizeLookupLimit(100); got != 50 {
		t.Fatalf("max = %d", got)
	}
}

func TestNormalizeLookupQuery(t *testing.T) {
	if got := normalizeLookupQuery("  advoc  "); got != "advoc" {
		t.Fatalf("trim = %q", got)
	}
}
