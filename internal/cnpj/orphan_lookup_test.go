package cnpj_test

import (
	"context"
	"errors"
	"testing"

	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgtype"

	"busca-cnpj-2026/internal/cnpj"
	"busca-cnpj-2026/internal/config"
	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
	"busca-cnpj-2026/internal/services"
)

func TestLookupServiceReturnsWhenRazaoSocialMissing(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	q := &mockQueries{
		est: cnpjdb.GetEstabelecimentoByCNPJRow{
			CnpjCompleto:        pgtype.Text{String: "77294254004343", Valid: true},
			CnpjBasico:          "77294254",
			NomeFantasia:        pgtype.Text{String: "AMAGGI", Valid: true},
			Uf:                  pgtype.Text{String: "MT", Valid: true},
			MunicipioNome:       "SORRISO",
			CnaeFiscalPrincipal: pgtype.Text{String: "4622200", Valid: true},
			RazaoSocial:         "", // orphan empresa row
		},
	}
	svc := cnpj.NewLookupService(q, services.NewCacheService())
	resp, err := svc.Lookup(context.Background(), "77294254004343")
	if err != nil {
		t.Fatal(err)
	}
	if resp.CNPJ != "77294254004343" {
		t.Fatalf("cnpj = %q", resp.CNPJ)
	}
	if resp.UF != "MT" {
		t.Fatalf("uf = %q", resp.UF)
	}
	if resp.RazaoSocial != "" {
		t.Fatalf("expected empty razao for orphan, got %q", resp.RazaoSocial)
	}
}

func TestLookupServiceStillNotFoundOnNoEstabelecimento(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	q := &mockQueries{estErr: pgx.ErrNoRows}
	svc := cnpj.NewLookupService(q, services.NewCacheService())
	_, err := svc.Lookup(context.Background(), "75315333033465")
	if !errors.Is(err, cnpj.ErrNotFound) {
		t.Fatalf("want ErrNotFound, got %v", err)
	}
}
