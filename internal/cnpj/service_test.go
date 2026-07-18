package cnpj_test

import (
	"context"
	"errors"
	"testing"

	"github.com/alicebob/miniredis/v2"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgtype"
	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/cnpj"
	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
	"busca-cnpj-2026/internal/services"
)

type mockQueries struct {
	est       cnpjdb.GetEstabelecimentoByCNPJRow
	estErr    error
	socios    []cnpjdb.ListSociosByBasicoRow
	simples   cnpjdb.GetSimplesByBasicoRow
	estCalls  int
	socioCall int
}

func (m *mockQueries) GetEstabelecimentoByCNPJ(
	_ context.Context, _ pgtype.Text,
) (cnpjdb.GetEstabelecimentoByCNPJRow, error) {
	m.estCalls++
	if m.estErr != nil {
		return cnpjdb.GetEstabelecimentoByCNPJRow{}, m.estErr
	}
	return m.est, nil
}
func (m *mockQueries) GetEmpresaByBasico(context.Context, string) (cnpjdb.GetEmpresaByBasicoRow, error) {
	return cnpjdb.GetEmpresaByBasicoRow{}, pgx.ErrNoRows
}
func (m *mockQueries) ListSociosByBasico(context.Context, string) ([]cnpjdb.ListSociosByBasicoRow, error) {
	m.socioCall++
	return m.socios, nil
}
func (m *mockQueries) GetSimplesByBasico(context.Context, string) (cnpjdb.GetSimplesByBasicoRow, error) {
	return m.simples, nil
}

func TestLookupServiceBuildsDTO(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	q := &mockQueries{
		est: cnpjdb.GetEstabelecimentoByCNPJRow{
			CnpjCompleto:        pgtype.Text{String: "00000000000191", Valid: true},
			CnpjBasico:          "00000000",
			NomeFantasia:        pgtype.Text{String: "FANTASIA", Valid: true},
			Uf:                  pgtype.Text{String: "DF", Valid: true},
			MunicipioNome:       "BRASILIA",
			CnaeFiscalPrincipal: pgtype.Text{String: "6422100", Valid: true},
			CnaeDescricao:       "Banco",
			RazaoSocial:         "BANCO DO BRASIL SA",
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

func TestLookupServiceNegativeCachesNotFound(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(mr.Close)
	database.RedisClient = redis.NewClient(&redis.Options{Addr: mr.Addr()})
	t.Cleanup(func() { database.RedisClient = nil })

	config.AppConfig = &config.Config{
		Cache: config.CacheConfig{Enabled: true, L1Enabled: false, TTLCNPJ: 86400, TTL: 300},
	}
	q := &mockQueries{estErr: pgx.ErrNoRows}
	svc := cnpj.NewLookupService(q, services.NewCacheService())

	_, err = svc.Lookup(context.Background(), "37511144000112")
	if !errors.Is(err, cnpj.ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
	if q.estCalls != 1 {
		t.Fatalf("estCalls after miss = %d", q.estCalls)
	}

	_, err = svc.Lookup(context.Background(), "37511144000112")
	if !errors.Is(err, cnpj.ErrNotFound) {
		t.Fatalf("second call want ErrNotFound, got %v", err)
	}
	if q.estCalls != 1 {
		t.Fatalf("negative cache should skip DB; estCalls=%d", q.estCalls)
	}
}
