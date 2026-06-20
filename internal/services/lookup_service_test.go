package services

import "testing"

func TestSearchUFFilter(t *testing.T) {
	svc := NewLookupService()
	all := svc.SearchUF("")
	if len(all) != 27 {
		t.Fatalf("expected 27 states, got %d", len(all))
	}
	filtered := svc.SearchUF("paulo")
	if len(filtered) == 0 {
		t.Fatal("expected SP match for paulo")
	}
}
