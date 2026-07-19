package cnpj_test

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

type e2eFixture struct {
	Cases []struct {
		CNPJ       string  `json:"cnpj"`
		UF         *string `json:"uf"`
		ExpectHTTP int     `json:"expect_http"`
	} `json:"cases"`
}

func TestE2EFixtureHasFiftyCNPJsAndAllUFs(t *testing.T) {
	path := filepath.Join("..", "..", "testdata", "e2e", "cnpj_lookup_50.json")
	if _, err := os.Stat(path); err != nil {
		path = filepath.Join("testdata", "e2e", "cnpj_lookup_50.json")
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		// Resolve from module root via cwd walk.
		wd, _ := os.Getwd()
		for i := 0; i < 5; i++ {
			candidate := filepath.Join(wd, "testdata", "e2e", "cnpj_lookup_50.json")
			if raw, err = os.ReadFile(candidate); err == nil {
				break
			}
			wd = filepath.Dir(wd)
		}
		if err != nil {
			t.Fatalf("read fixture: %v", err)
		}
	}
	var fx e2eFixture
	if err := json.Unmarshal(raw, &fx); err != nil {
		t.Fatal(err)
	}
	if len(fx.Cases) != 50 {
		t.Fatalf("cases=%d want 50", len(fx.Cases))
	}
	required := []string{
		"AC", "AL", "AP", "AM", "BA", "CE", "DF", "ES", "GO", "MA", "MT", "MS", "MG",
		"PA", "PB", "PR", "PE", "PI", "RJ", "RN", "RS", "RO", "RR", "SC", "SP", "SE", "TO",
	}
	seen := map[string]bool{}
	for _, c := range fx.Cases {
		if len(c.CNPJ) != 14 {
			t.Fatalf("bad cnpj length %q", c.CNPJ)
		}
		if c.UF != nil {
			seen[*c.UF] = true
		}
		if c.ExpectHTTP != 200 && c.ExpectHTTP != 404 && c.ExpectHTTP != 400 {
			t.Fatalf("unexpected expect_http=%d for %s", c.ExpectHTTP, c.CNPJ)
		}
	}
	for _, uf := range required {
		if !seen[uf] {
			t.Fatalf("fixture missing UF %s", uf)
		}
	}
}
