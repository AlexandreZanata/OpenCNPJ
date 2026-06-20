package middleware

import (
	"testing"

	"busca-cnpj-2026/internal/config"
)

func TestRateLimiterUsesConfig(t *testing.T) {
	config.AppConfig = &config.Config{
		Server: config.ServerConfig{
			RateLimitMax:           500,
			RateLimitWindowSeconds: 30,
		},
	}

	handler := RateLimiter()
	if handler == nil {
		t.Fatal("RateLimiter returned nil handler")
	}
}

func TestRateLimiterDefaultsWhenZero(t *testing.T) {
	config.AppConfig = &config.Config{
		Server: config.ServerConfig{
			RateLimitMax:           0,
			RateLimitWindowSeconds: 0,
		},
	}

	handler := RateLimiter()
	if handler == nil {
		t.Fatal("RateLimiter returned nil handler with zero config")
	}
}
