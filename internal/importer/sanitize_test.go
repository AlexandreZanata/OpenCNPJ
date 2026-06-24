package importer

import "testing"

func TestCleanTextRemovesNullBytes(t *testing.T) {
	got := cleanText("abc\x00def")
	if got != "abcdef" {
		t.Fatalf("cleanText() = %q, want abcdef", got)
	}
}

func TestNullIfEmpty(t *testing.T) {
	if nullIfEmpty("") != nil {
		t.Fatal("empty string should map to nil")
	}
	if nullIfEmpty("x") != "x" {
		t.Fatal("non-empty should pass through")
	}
}
