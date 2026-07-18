package services

import (
	"testing"
	"time"

	"busca-cnpj-2026/internal/config"
)

func TestCacheTTLForKey(t *testing.T) {
	config.AppConfig = &config.Config{
		Cache: config.CacheConfig{
			TTL:          300,
			TTLCNPJ:      86400,
			TTLSearch:    300,
			TTLAnalytics: 3600,
			TTLLookup:    900,
		},
	}
	profile := newCacheTTLProfile()

	cases := []struct {
		key  string
		want time.Duration
	}{
		{"estabelecimento:cnpj:v2:123", 24 * time.Hour},
		{"public:cnpj:v2:00000000000191", 24 * time.Hour},
		{"empresas:search:v4:abc", 5 * time.Minute},
		{"estabelecimentos:search:v4:abc", 5 * time.Minute},
		{"stats:uf", time.Hour},
		{"lookup:cnae:v2:x:10", 15 * time.Minute},
	}

	for _, tc := range cases {
		if got := profile.forKey(tc.key); got != tc.want {
			t.Fatalf("forKey(%q) = %v, want %v", tc.key, got, tc.want)
		}
	}
}
