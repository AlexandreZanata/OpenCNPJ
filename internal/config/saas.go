package config

import (
	"fmt"
	"net/url"
	"os"
	"strconv"
	"strings"

	"github.com/spf13/viper"
)

// PoolConfig holds PostgreSQL connection pool limits.
type PoolConfig struct {
	MaxOpenConns int
	MaxIdleConns int
}

// SaasConfig toggles SaaS deployment mode (dual DB, admin, public API).
type SaasConfig struct {
	Enabled              bool
	PublicAPIOnly        bool
	AdminEnabled         bool
	AdminJWTTTLMinutes   int
	AdminRefreshTTLDays  int
	MFARequired          bool
	MFATOTPIssuer        string
	MFABypassCode        string
	DefaultClientRateMin int
	DefaultMonthlyQuota  int
	DocsEnabled          bool
	DocsPublicURL        string
	AdminHost            string
}

// DatabaseURLConfig is a Postgres URL plus pool settings (SaaS VPS layout).
type DatabaseURLConfig struct {
	URL          string
	MaxOpenConns int
	MaxIdleConns int
}

func loadSaasFromViper() (SaasConfig, DatabaseURLConfig, DatabaseURLConfig) {
	saas := SaasConfig{
		Enabled:              viper.GetBool("saas.enabled"),
		PublicAPIOnly:        viper.GetBool("saas.public_api_only"),
		AdminEnabled:         viper.GetBool("saas.admin_enabled"),
		AdminJWTTTLMinutes:   viper.GetInt("saas.admin_jwt_ttl_minutes"),
		AdminRefreshTTLDays:  viper.GetInt("saas.admin_refresh_ttl_days"),
		MFARequired:          viper.GetBool("saas.mfa_required"),
		MFATOTPIssuer:        viper.GetString("saas.mfa_totp_issuer"),
		MFABypassCode:        viper.GetString("saas.mfa_bypass_code"),
		DefaultClientRateMin: viper.GetInt("saas.default_client_rate_per_min"),
		DefaultMonthlyQuota:  viper.GetInt("saas.default_monthly_quota"),
		DocsEnabled:          viper.GetBool("saas.docs_enabled"),
		DocsPublicURL:        viper.GetString("saas.docs_public_url"),
		AdminHost:            viper.GetString("saas.admin_host"),
	}

	cnpj := DatabaseURLConfig{
		URL:          firstNonEmpty(os.Getenv("CNPJ_DATABASE_URL"), viper.GetString("database_cnpj.url")),
		MaxOpenConns: viper.GetInt("database_cnpj.max_open_conns"),
		MaxIdleConns: viper.GetInt("database_cnpj.max_idle_conns"),
	}

	saasDB := DatabaseURLConfig{
		URL:          firstNonEmpty(os.Getenv("SAAS_DATABASE_URL"), viper.GetString("database_saas.url")),
		MaxOpenConns: viper.GetInt("database_saas.max_open_conns"),
		MaxIdleConns: viper.GetInt("database_saas.max_idle_conns"),
	}

	return saas, cnpj, saasDB
}

func applySaasRateLimitDefaults() {
	// Values merged in ServerConfig via firstNonZero during Load.
}

func resolveRateLimitMax() int {
	if viper.IsSet("rate_limit.max_requests") {
		return viper.GetInt("rate_limit.max_requests")
	}
	return viper.GetInt("server.rate_limit_max")
}

func resolveRateLimitWindow() int {
	if viper.IsSet("rate_limit.window_seconds") {
		return viper.GetInt("rate_limit.window_seconds")
	}
	return viper.GetInt("server.rate_limit_window_seconds")
}

// GetCNPJDatabaseURL returns the CNPJ consulta DSN (URL or key=value form).
func GetCNPJDatabaseURL() string {
	if u := strings.TrimSpace(os.Getenv("CNPJ_DATABASE_URL")); u != "" {
		return normalizePostgresDSN(u)
	}
	if AppConfig == nil {
		return ""
	}
	if AppConfig.SaaS.Enabled && AppConfig.DatabaseCNPJ.URL != "" {
		return normalizePostgresDSN(AppConfig.DatabaseCNPJ.URL)
	}
	return GetDSN()
}

// GetSaaSDatabaseURL returns the SaaS metadata DSN.
func GetSaaSDatabaseURL() string {
	if u := strings.TrimSpace(os.Getenv("SAAS_DATABASE_URL")); u != "" {
		return normalizePostgresDSN(u)
	}
	if AppConfig == nil || !AppConfig.SaaS.Enabled {
		return ""
	}
	return normalizePostgresDSN(AppConfig.DatabaseSaaS.URL)
}

// GetRedisURL returns a redis:// URL when configured, else empty.
func GetRedisURL() string {
	if AppConfig == nil {
		return ""
	}
	if u := strings.TrimSpace(AppConfig.Redis.URL); u != "" {
		return u
	}
	if u := strings.TrimSpace(os.Getenv("REDIS_URL")); u != "" {
		return u
	}
	return ""
}

func normalizePostgresDSN(raw string) string {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return ""
	}
	if strings.HasPrefix(raw, "postgres://") || strings.HasPrefix(raw, "postgresql://") {
		return raw
	}
	return raw
}

func parseRedisURL(raw string) (host string, port int, password string, db int, err error) {
	u, err := url.Parse(raw)
	if err != nil {
		return "", 0, "", 0, fmt.Errorf("parse redis url: %w", err)
	}
	host = u.Hostname()
	if host == "" {
		return "", 0, "", 0, fmt.Errorf("redis url missing host")
	}
	port = 6379
	if p := u.Port(); p != "" {
		port, err = strconv.Atoi(p)
		if err != nil {
			return "", 0, "", 0, fmt.Errorf("redis url invalid port: %w", err)
		}
	}
	if u.User != nil {
		password, _ = u.User.Password()
	}
	db = 0
	if path := strings.TrimPrefix(u.Path, "/"); path != "" {
		db, err = strconv.Atoi(path)
		if err != nil {
			return "", 0, "", 0, fmt.Errorf("redis url invalid db index: %w", err)
		}
	}
	return host, port, password, db, nil
}

func firstNonEmpty(values ...string) string {
	for _, v := range values {
		if strings.TrimSpace(v) != "" {
			return strings.TrimSpace(v)
		}
	}
	return ""
}
