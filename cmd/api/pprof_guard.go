package main

import (
	"os"

	"busca-cnpj-2026/internal/config"
)

// pprofAllowed reports whether the debug pprof server may start.
func pprofAllowed() bool {
	if config.AppConfig != nil && config.AppConfig.SaaS.Enabled && config.AppConfig.SaaS.PublicAPIOnly {
		return false
	}
	if os.Getenv("ENABLE_PPROF") == "true" {
		return true
	}
	return config.AppConfig != nil && config.AppConfig.Logging.Level == "debug"
}
