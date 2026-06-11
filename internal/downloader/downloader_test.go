package downloader

import (
	"testing"
	"time"
)

func TestResolveMonth_Explicit(t *testing.T) {
	available := []string{"2025-11", "2025-12", "2026-05"}
	month, fallback, err := ResolveMonth(available, "2026-05", time.Date(2026, 6, 11, 0, 0, 0, 0, time.UTC))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if month != "2026-05" || fallback {
		t.Fatalf("got month=%s fallback=%v", month, fallback)
	}
}

func TestResolveMonth_CurrentWhenAvailable(t *testing.T) {
	available := []string{"2026-04", "2026-05", "2026-06"}
	now := time.Date(2026, 6, 11, 0, 0, 0, 0, time.UTC)
	month, fallback, err := ResolveMonth(available, "", now)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if month != "2026-06" || fallback {
		t.Fatalf("got month=%s fallback=%v", month, fallback)
	}
}

func TestResolveMonth_FallbackToLatest(t *testing.T) {
	available := []string{"2026-03", "2026-04", "2026-05"}
	now := time.Date(2026, 6, 11, 0, 0, 0, 0, time.UTC)
	month, fallback, err := ResolveMonth(available, "", now)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if month != "2026-05" || !fallback {
		t.Fatalf("got month=%s fallback=%v", month, fallback)
	}
}

func TestResolveMonth_NotFound(t *testing.T) {
	available := []string{"2026-05"}
	_, _, err := ResolveMonth(available, "2020-01", time.Now())
	if err == nil {
		t.Fatal("expected error for unavailable month")
	}
}

func TestIsCNPJMember(t *testing.T) {
	cases := map[string]bool{
		"K3241.K03200Y0.D30512.EMPRECSV": true,
		"readme.txt":                     false,
		"F.K03200$Z.D40614.SOCIOCSV":     true,
	}
	for name, want := range cases {
		if got := isCNPJMember(name); got != want {
			t.Fatalf("%s: got %v want %v", name, got, want)
		}
	}
}

func TestSplitFiles(t *testing.T) {
	ref, data := splitFiles([]string{"Socios0.zip", "Cnaes.zip", "Empresas0.zip", "Motivos.zip"})
	if len(ref) != 2 || ref[0] != "Cnaes.zip" || ref[1] != "Motivos.zip" {
		t.Fatalf("unexpected reference files: %v", ref)
	}
	if len(data) != 2 || data[0] != "Empresas0.zip" || data[1] != "Socios0.zip" {
		t.Fatalf("unexpected data files: %v", data)
	}
}
