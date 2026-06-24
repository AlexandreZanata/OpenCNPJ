package repository

import (
	"strings"
	"testing"
)

func TestBackfillMunicipiosUFSQL(t *testing.T) {
	if !strings.Contains(backfillMunicipiosUFSQL, "UPDATE municipios") {
		t.Fatal("expected municipios update statement")
	}
	if !strings.Contains(backfillMunicipiosUFSQL, "estabelecimentos") {
		t.Fatal("expected estabelecimentos source")
	}
}
