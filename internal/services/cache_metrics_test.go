package services

import "testing"

func TestCacheKeyPrefix(t *testing.T) {
	if got := cacheKeyPrefix("empresas:search:v4:abc123"); got != "empresas" {
		t.Fatalf("prefix = %q, want empresas", got)
	}
	if got := cacheKeyPrefix("stats"); got != "stats" {
		t.Fatalf("prefix = %q, want stats", got)
	}
}

func TestRecordCacheMetricsNoPanic(_ *testing.T) {
	recordCacheHit("empresas:search:v4:deadbeef")
	recordCacheMiss("estabelecimentos:search:v4:cafebabe")
	recordL1CacheHit("estabelecimento:cnpj:v2:1")
	recordL1CacheMiss("estabelecimento:cnpj:v2:2")
}
