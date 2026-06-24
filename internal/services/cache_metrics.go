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
			Help: "Total Redis cache hits by key prefix",
		},
		[]string{"key_prefix"},
	)

	cacheMissesTotal = promauto.NewCounterVec(
		prometheus.CounterOpts{
			Name: "busca_cnpj_cache_misses_total",
			Help: "Total Redis cache misses by key prefix",
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
