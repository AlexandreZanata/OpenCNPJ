package handlers

import "testing"

func TestParseStatsLimit(t *testing.T) {
	if got := parseStatsLimit("", 15); got != 15 {
		t.Fatalf("fallback = %d, want 15", got)
	}
	if got := parseStatsLimit("25", 15); got != 25 {
		t.Fatalf("valid limit = %d, want 25", got)
	}
	if got := parseStatsLimit("0", 15); got != 15 {
		t.Fatalf("zero limit should fallback, got %d", got)
	}
	if got := parseStatsLimit("5000", 15); got != 15 {
		t.Fatalf("over max should fallback, got %d", got)
	}
}
