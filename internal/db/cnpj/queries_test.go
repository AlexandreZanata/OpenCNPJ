package cnpjdb_test

import (
	"os"
	"strings"
	"testing"
)

func TestGetEstabelecimentoQueryUsesCnpjCompletoPredicate(t *testing.T) {
	raw, err := os.ReadFile("../../../db/queries/cnpj/estabelecimento.sql")
	if err != nil {
		t.Fatal(err)
	}
	sql := string(raw)
	if !strings.Contains(sql, "WHERE e.cnpj_completo = $1") {
		t.Fatal("hot-path query must filter on cnpj_completo")
	}
}
