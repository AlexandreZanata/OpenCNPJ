package downloader

import (
	"strings"
	"testing"
)

func TestFormatProgressLine_withTotal(t *testing.T) {
	line := formatProgressLine(3, 10, "Empresas0.zip", 5_000_000, 10_000_000)
	if line == "" {
		t.Fatal("expected non-empty line")
	}
	for _, part := range []string{"3/10", "Empresas0.zip", "4.8 MB"} {
		if !strings.Contains(line, part) {
			t.Fatalf("line missing %q: %s", part, line)
		}
	}
}

func TestHumanBytes(t *testing.T) {
	if humanBytes(512) != "512 B" {
		t.Fatalf("got %s", humanBytes(512))
	}
	if humanBytes(1536) != "1.5 KB" {
		t.Fatalf("got %s", humanBytes(1536))
	}
}
