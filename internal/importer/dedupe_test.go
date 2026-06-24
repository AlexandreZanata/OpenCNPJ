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

func TestDedupeKeyEmpresas(t *testing.T) {
	key, ok := dedupeKey("empresas", []any{"08314885", "ACME"})
	if !ok || key != "08314885" {
		t.Fatalf("dedupeKey empresas = (%q, %v)", key, ok)
	}
}

func TestDedupeKeySocios(t *testing.T) {
	row := []any{"12345678", "1", "NAME", "cpf", "49", nil}
	key, ok := dedupeKey("socios", row)
	if !ok || key == "" {
		t.Fatalf("dedupeKey socios = (%q, %v)", key, ok)
	}
}
