package repository

import "testing"

func TestParseEstabIDsFromStrings(t *testing.T) {
	ids, err := ParseEstabIDsFromStrings([]string{"1", "42"})
	if err != nil {
		t.Fatalf("parse: %v", err)
	}
	if len(ids) != 2 || ids[0] != 1 || ids[1] != 42 {
		t.Fatalf("ids = %#v", ids)
	}
	_, err = ParseEstabIDsFromStrings([]string{"bad"})
	if err == nil {
		t.Fatal("expected error")
	}
}
