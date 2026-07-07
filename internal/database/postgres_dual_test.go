package database

import (
	"context"
	"testing"

	"busca-cnpj-2026/internal/config"
)

func TestPostgresReadySingleDB(t *testing.T) {
	config.AppConfig = &config.Config{SaaS: config.SaasConfig{Enabled: false}}
	if PostgresReady(context.Background()) {
		t.Fatal("expected false when DB is nil")
	}
}

func TestPostgresReadyRequiresSaaSDB(t *testing.T) {
	config.AppConfig = &config.Config{SaaS: config.SaasConfig{Enabled: true}}
	DB = nil
	DBSaaS = nil
	if PostgresReady(context.Background()) {
		t.Fatal("expected false when pools are nil")
	}
}
