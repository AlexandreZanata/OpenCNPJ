package importer

import "testing"

func TestDedupeSet(t *testing.T) {
	d := NewDedupeSet()
	if d.Seen("a") {
		t.Fatal("first key should not be seen")
	}
	if !d.Seen("a") {
		t.Fatal("duplicate should be seen")
	}
}

func TestSocioDedupeKey(t *testing.T) {
	row := []any{"12345678", "1", "NAME", "cpf", "49", nil}
	if socioDedupeKey(row) == "" {
		t.Fatal("expected key")
	}
}
