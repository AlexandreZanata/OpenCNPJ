package services

import (
	"context"
	"testing"
	"time"

	"busca-cnpj-2026/internal/cache/l1"
	"busca-cnpj-2026/internal/config"
)

func TestFetchCachedBytesL1Hit(t *testing.T) {
	config.AppConfig = &config.Config{Cache: config.CacheConfig{TTL: 300}}
	l1c, err := l1.New(l1.Config{MaxCostMB: 1, NumCounters: 1000, BufferItems: 8})
	if err != nil {
		t.Fatalf("l1.New: %v", err)
	}
	defer l1c.Close()

	payload := []byte{cacheFormatMsgpack, 0x01}
	l1c.SetWithTTL("estabelecimento:cnpj:v2:1", payload, time.Minute)
	l1c.Wait()

	svc := &CacheService{
		enabled: true,
		ttl:     newCacheTTLProfile(),
		l1:      l1c,
	}

	data, hit, err := svc.fetchCachedBytes(context.Background(), "estabelecimento:cnpj:v2:1")
	if err != nil {
		t.Fatalf("fetchCachedBytes err: %v", err)
	}
	if !hit {
		t.Fatal("expected L1 hit")
	}
	if len(data) != len(payload) {
		t.Fatalf("data len = %d, want %d", len(data), len(payload))
	}
}

func TestNewCacheServiceL1WhenEnabled(t *testing.T) {
	config.AppConfig = &config.Config{
		Cache: config.CacheConfig{
			Enabled:       true,
			L1Enabled:     true,
			L1MaxCostMB:   1,
			L1NumCounters: 1000,
			L1BufferItems: 8,
			TTL:           300,
		},
	}
	svc := NewCacheService()
	if svc.l1 == nil {
		t.Fatal("expected L1 cache when l1_enabled=true")
	}
	svc.l1.Close()
}

func TestNewCacheServiceL1Disabled(t *testing.T) {
	config.AppConfig = &config.Config{
		Cache: config.CacheConfig{Enabled: true, L1Enabled: false, TTL: 300},
	}
	svc := NewCacheService()
	if svc.l1 != nil {
		t.Fatal("expected nil L1 when l1_enabled=false")
	}
}
