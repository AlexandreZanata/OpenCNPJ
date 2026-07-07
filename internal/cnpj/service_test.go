package cnpj_test

import (
	"context"
	"testing"

	"github.com/jackc/pgx/v5/pgtype"

	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
	"busca-cnpj-2026/internal/cnpj"
	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/services"
)

type mockQueries struct {
	est     cnpjdb.GetEstabelecimentoByCNPJRow
	emp     cnpjdb.GetEmpresaByBasicoRow
	socios  []cnpjdb.ListSociosByBasicoRow
	simples cnpjdb.GetSimplesByBasicoRow
}

func (m *mockQueries) GetEstabelecimentoByCNPJ(context.Context, pgtype.Text) (cnpjdb.GetEstabelecimentoByCNPJRow, error) {
	return m.est, nil
}
func (m *mockQueries) GetEmpresaByBasico(context.Context, string) (cnpjdb.GetEmpresaByBasicoRow, error) {
	return m.emp, nil
}
func (m *mockQueries) ListSociosByBasico(context.Context, string) ([]cnpjdb.ListSociosByBasicoRow, error) {
	return m.socios, nil
}
func (m *mockQueries) GetSimplesByBasico(context.Context, string) (cnpjdb.GetSimplesByBasicoRow, error) {
	return m.simples, nil
}

func TestLookupServiceBuildsDTO(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	q := &mockQueries{
		est: cnpjdb.GetEstabelecimentoByCNPJRow{
			CnpjCompleto: pgtype.Text{String: "00000000000191", Valid: true},
			CnpjBasico:   "00000000",
			NomeFantasia: pgtype.Text{String: "FANTASIA", Valid: true},
			Uf:           pgtype.Text{String: "DF", Valid: true},
			MunicipioNome: "BRASILIA",
			CnaeFiscalPrincipal: pgtype.Text{String: "6422100", Valid: true},
			CnaeDescricao: "Banco",
			RazaoSocial:   "BANCO DO BRASIL SA",
		},
		emp: cnpjdb.GetEmpresaByBasicoRow{
			CnpjBasico:  "00000000",
			RazaoSocial: "BANCO DO BRASIL SA",
		},
	}
	svc := cnpj.NewLookupService(q, services.NewCacheService())
	resp, err := svc.Lookup(context.Background(), "00000000000191")
	if err != nil {
		t.Fatal(err)
	}
	if resp.CNPJ != "00000000000191" {
		t.Fatalf("cnpj = %q", resp.CNPJ)
	}
	if resp.RazaoSocial != "BANCO DO BRASIL SA" {
		t.Fatalf("razao = %q", resp.RazaoSocial)
	}
}
