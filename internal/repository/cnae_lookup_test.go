package repository

import (
	"strings"
	"testing"
)

func TestSplitLookupTerms(t *testing.T) {
	tests := []struct {
		query string
		want  []string
	}{
		{query: "", want: nil},
		{query: "4712100", want: []string{"4712100"}},
		{query: "mercado varejo", want: []string{"mercado", "varejo"}},
		{query: "a", want: nil},
		{query: "ab", want: []string{"ab"}},
	}
	for _, tc := range tests {
		got := splitLookupTerms(tc.query)
		if len(got) != len(tc.want) {
			t.Fatalf("splitLookupTerms(%q) = %v, want %v", tc.query, got, tc.want)
		}
		for i := range got {
			if got[i] != tc.want[i] {
				t.Fatalf("splitLookupTerms(%q)[%d] = %q, want %q", tc.query, i, got[i], tc.want[i])
			}
		}
	}
}

func TestFoldAccents(t *testing.T) {
	if got := foldAccents(" Comércio "); got != "comercio" {
		t.Fatalf("foldAccents = %q", got)
	}
}

func TestBuildCNAEDescricaoMatch(t *testing.T) {
	args := make([]any, 0, 4)
	pos := 1
	clause := buildCNAEDescricaoMatch([]string{"mercado", "varejo"}, &pos, &args)
	if !strings.Contains(clause, "translate(lower(descricao)") {
		t.Fatalf("expected accent fold in clause: %q", clause)
	}
	if len(args) != 2 || args[0] != "%mercado%" || args[1] != "%varejo%" {
		t.Fatalf("args = %#v", args)
	}
}

func TestCnaeLookupMinLen(t *testing.T) {
	if !cnaeLookupMinLen("47") {
		t.Fatal("expected digits to pass")
	}
	if cnaeLookupMinLen("a") {
		t.Fatal("expected single letter to fail")
	}
	if !cnaeLookupMinLen("ab") {
		t.Fatal("expected two letters to pass")
	}
}
