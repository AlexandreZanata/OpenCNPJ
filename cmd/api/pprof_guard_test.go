package main

import (
	"testing"

	"busca-cnpj-2026/internal/config"
)

func TestPprofAllowedPublicAPIOnlyBlocks(t *testing.T) {
	config.AppConfig = &config.Config{
		SaaS:    config.SaasConfig{Enabled: true, PublicAPIOnly: true},
		Logging: config.LoggingConfig{Level: "debug"},
	}
	t.Setenv("ENABLE_PPROF", "")
	if pprofAllowed() {
		t.Fatal("pprof must be disabled when public_api_only is enabled")
	}
}

func TestPprofAllowedDebugWhenNotPublicOnly(t *testing.T) {
	config.AppConfig = &config.Config{
		SaaS:    config.SaasConfig{Enabled: false},
		Logging: config.LoggingConfig{Level: "debug"},
	}
	if !pprofAllowed() {
		t.Fatal("pprof should be allowed in debug mode without public_api_only")
	}
}

func TestPprofAllowedEnvOverride(t *testing.T) {
	config.AppConfig = &config.Config{
		SaaS:    config.SaasConfig{Enabled: true, PublicAPIOnly: false},
		Logging: config.LoggingConfig{Level: "info"},
	}
	t.Setenv("ENABLE_PPROF", "true")
	if !pprofAllowed() {
		t.Fatal("ENABLE_PPROF should allow pprof")
	}
}
