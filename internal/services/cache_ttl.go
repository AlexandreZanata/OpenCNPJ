package services

import (
	"strings"
	"time"

	"busca-cnpj-2026/internal/config"
)

const (
	defaultTTLCNPJ      = 24 * time.Hour
	defaultTTLSearch    = 5 * time.Minute
	defaultTTLAnalytics = 1 * time.Hour
	defaultTTLLookup    = 15 * time.Minute
)

type cacheTTLProfile struct {
	defaultTTL time.Duration
	cnpjTTL    time.Duration
	searchTTL  time.Duration
	statsTTL   time.Duration
	lookupTTL  time.Duration
}

func newCacheTTLProfile() cacheTTLProfile {
	fallback := time.Duration(config.AppConfig.Cache.TTL) * time.Second
	if fallback == 0 {
		fallback = defaultTTLSearch
	}

	return cacheTTLProfile{
		defaultTTL: fallback,
		cnpjTTL:    secondsOrDefault(config.AppConfig.Cache.TTLCNPJ, defaultTTLCNPJ),
		searchTTL:  secondsOrDefault(config.AppConfig.Cache.TTLSearch, defaultTTLSearch),
		statsTTL:   secondsOrDefault(config.AppConfig.Cache.TTLAnalytics, defaultTTLAnalytics),
		lookupTTL:  secondsOrDefault(config.AppConfig.Cache.TTLLookup, defaultTTLLookup),
	}
}

func secondsOrDefault(seconds int, fallback time.Duration) time.Duration {
	if seconds <= 0 {
		return fallback
	}
	return time.Duration(seconds) * time.Second
}

func (p cacheTTLProfile) forKey(key string) time.Duration {
	prefix := cacheKeyPrefix(key)
	switch {
	case prefix == "estabelecimento":
		return p.cnpjTTL
	case prefix == "empresas" || prefix == "estabelecimentos":
		return p.searchTTL
	case prefix == "stats" || strings.HasPrefix(key, "stats:"):
		return p.statsTTL
	case prefix == "lookup" || strings.HasPrefix(key, "lookup:"):
		return p.lookupTTL
	default:
		return p.defaultTTL
	}
}
