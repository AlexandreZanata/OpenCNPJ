package cnpj_test

import (
	"context"
	"testing"

	"github.com/jackc/pgx/v5/pgtype"

	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
)

// TestIntegration_GetEstabelecimentoWithoutEmpresa ensures LEFT JOIN returns
// the establishment when empresas row is missing (false-404 regression).
func TestIntegration_GetEstabelecimentoWithoutEmpresa(t *testing.T) {
	if testing.Short() {
		t.Skip("integration test")
	}
	ctx := context.Background()
	pool, cleanup := startCNPJSchema(t, ctx)
	defer cleanup()

	_, err := pool.Exec(ctx, `
		ALTER TABLE estabelecimentos DROP CONSTRAINT IF EXISTS estabelecimentos_cnpj_basico_fkey;
		INSERT INTO estabelecimentos (
			uuid_id, cnpj_basico, cnpj_ordem, cnpj_dv, nome_fantasia, situacao_cadastral, uf
		) VALUES (
			gen_random_uuid(), '77294254', '0043', '43', 'AMAGGI', '02', 'MT'
		);
	`)
	if err != nil {
		t.Fatal(err)
	}

	q := cnpjdb.New(pool)
	row, err := q.GetEstabelecimentoByCNPJ(ctx, pgtype.Text{String: "77294254004343", Valid: true})
	if err != nil {
		t.Fatalf("LEFT JOIN lookup failed: %v", err)
	}
	if row.CnpjBasico != "77294254" {
		t.Fatalf("basico = %q", row.CnpjBasico)
	}
	if row.RazaoSocial != "" {
		t.Fatalf("razao want empty, got %q", row.RazaoSocial)
	}
	if row.Uf.String != "MT" {
		t.Fatalf("uf = %q", row.Uf.String)
	}
}
