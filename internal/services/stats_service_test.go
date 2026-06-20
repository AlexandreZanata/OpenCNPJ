package services

import (
	"context"
	"testing"

	"busca-cnpj-2026/internal/config"
)

func TestStatsServiceCacheKeyFormat(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{Enabled: false}}
	svc := NewStatsService()
	if svc == nil {
		t.Fatal("NewStatsService returned nil")
	}

	key := "stats:analytics:15:10"
	_, err := GetOrSetJSON(svc.cache, context.Background(), key, func() (*struct{ OK bool }, error) {
		return &struct{ OK bool }{OK: true}, nil
	})
	if err != nil {
		t.Fatalf("GetOrSetJSON: %v", err)
	}
}
