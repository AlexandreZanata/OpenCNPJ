package importer

import "testing"

func TestSkipDedupeNilSet(t *testing.T) {
	if skipDedupe("empresas", []any{"12345678"}, nil) {
		t.Fatal("nil dedupe set must not skip")
	}
}

func TestSkipDedupeDuplicate(t *testing.T) {
	d := NewDedupeSet()
	row := []any{"12345678", "ACME"}
	if skipDedupe("empresas", row, d) {
		t.Fatal("first row must not skip")
	}
	if !skipDedupe("empresas", row, d) {
		t.Fatal("duplicate row must skip")
	}
}
