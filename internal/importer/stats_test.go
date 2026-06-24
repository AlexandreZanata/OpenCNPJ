package importer

import (
	"testing"
	"time"
)

func TestStatsSummary(t *testing.T) {
	s := NewStats()
	s.StartedAt = time.Now().Add(-time.Second)
	s.Add("empresas", 1000)
	s.Add("empresas", 500)
	if s.TotalRows() != 1500 {
		t.Fatalf("got %d", s.TotalRows())
	}
	if s.Summary() == "" {
		t.Fatal("expected summary")
	}
}
