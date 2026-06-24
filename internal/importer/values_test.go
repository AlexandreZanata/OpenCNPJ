package importer

import "testing"

func TestSanitize(t *testing.T) {
	if got := sanitize("a\x00b"); got != "ab" {
		t.Fatalf("got %q", got)
	}
}

func TestFkOrNil(t *testing.T) {
	ok := func(v string) bool { return v == "01" }
	if got := fkOrNil(ok, "01"); got != "01" {
		t.Fatalf("expected 01")
	}
	if got := fkOrNil(ok, "99"); got != nil {
		t.Fatalf("expected nil for invalid fk")
	}
}
