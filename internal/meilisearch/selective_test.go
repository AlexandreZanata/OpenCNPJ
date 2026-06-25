package meilisearch

import (
	"strings"
	"testing"
)

func TestSelectiveSQLActiveMatriz(t *testing.T) {
	if !strings.Contains(SelectiveEstabSQL, "identificador_matriz_filial = '1'") {
		t.Fatal("SelectiveEstabSQL must filter active matriz")
	}
	if !strings.Contains(SelectiveEstabSQL, "situacao_cadastral = '02'") {
		t.Fatal("SelectiveEstabSQL must filter active situacao")
	}
	if !strings.Contains(SelectiveEmpresaSQL, "identificador_matriz_filial = '1'") {
		t.Fatal("SelectiveEmpresaSQL must require active matriz")
	}
}

func TestSyncOptionsSelectiveDefault(t *testing.T) {
	opts := SyncOptions{SelectiveActiveMatriz: true}
	if !opts.SelectiveActiveMatriz {
		t.Fatal("selective matriz expected")
	}
}
