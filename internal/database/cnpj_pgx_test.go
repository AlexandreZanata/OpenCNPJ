package database_test

import (
	"testing"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
)

func TestInitCNPJPgxSkippedWhenDisabled(t *testing.T) {
	config.AppConfig = &config.Config{SaaS: config.SaasConfig{Enabled: false}}
	if err := database.InitCNPJPgx(); err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if database.CNPJPool != nil {
		t.Fatal("pool should be nil when saas disabled")
	}
}
