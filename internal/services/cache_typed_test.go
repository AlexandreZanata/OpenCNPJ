package services

import (
	"context"
	"testing"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/models"
)

func TestGetOrSetJSONCacheDisabled(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	svc := NewCacheService()

	got, err := GetOrSetJSON(svc, context.Background(), "key", func() (*models.SearchResponse, error) {
		return &models.SearchResponse{Total: 42, Limit: 10}, nil
	})
	if err != nil {
		t.Fatalf("GetOrSetJSON: %v", err)
	}
	if got.Total != 42 {
		t.Fatalf("expected total 42, got %d", got.Total)
	}
}

func TestGetOrSetJSONPreservesStructShape(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	svc := NewCacheService()

	original := &models.EstabelecimentoCompleto{
		Estabelecimento: models.Estabelecimento{CNPJCompleto: "12345678000199"},
	}
	got, err := GetOrSetJSON(svc, context.Background(), "cnpj-key", func() (*models.EstabelecimentoCompleto, error) {
		return original, nil
	})
	if err != nil {
		t.Fatalf("GetOrSetJSON: %v", err)
	}
	if got.CNPJCompleto != original.CNPJCompleto {
		t.Fatalf("expected CNPJ %s, got %s", original.CNPJCompleto, got.CNPJCompleto)
	}
}
