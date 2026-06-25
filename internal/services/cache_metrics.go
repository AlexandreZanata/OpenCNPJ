package services

import (
	"strings"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
)

var (
	cacheHitsTotal = promauto.NewCounterVec(
		prometheus.CounterOpts{
			Name: "busca_cnpj_cache_hits_total",
			Help: "Total Redis L2 cache hits by key prefix",
		},
		[]string{"key_prefix"},
	)

	cacheMissesTotal = promauto.NewCounterVec(
		prometheus.CounterOpts{
			Name: "busca_cnpj_cache_misses_total",
			Help: "Total cache misses (L1+L2) by key prefix",
		},
		[]string{"key_prefix"},
	)

	l1CacheHitsTotal = promauto.NewCounterVec(
		prometheus.CounterOpts{
			Name: "busca_cnpj_l1_cache_hits_total",
			Help: "Total in-process Ristretto L1 cache hits by key prefix",
		},
		[]string{"key_prefix"},
	)

	l1CacheMissesTotal = promauto.NewCounterVec(
		prometheus.CounterOpts{
			Name: "busca_cnpj_l1_cache_misses_total",
			Help: "Total in-process Ristretto L1 cache misses by key prefix",
		},
		[]string{"key_prefix"},
	)
)

func cacheKeyPrefix(key string) string {
	if idx := strings.Index(key, ":"); idx > 0 {
		return key[:idx]
	}
	return key
}

func recordCacheHit(key string) {
	cacheHitsTotal.WithLabelValues(cacheKeyPrefix(key)).Inc()
}

func recordCacheMiss(key string) {
	cacheMissesTotal.WithLabelValues(cacheKeyPrefix(key)).Inc()
}

func recordL1CacheHit(key string) {
	l1CacheHitsTotal.WithLabelValues(cacheKeyPrefix(key)).Inc()
}

func recordL1CacheMiss(key string) {
	l1CacheMissesTotal.WithLabelValues(cacheKeyPrefix(key)).Inc()
}
