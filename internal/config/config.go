package config

import (
	"bufio"
	"fmt"
	"os"
	"runtime"
	"strings"

	"github.com/spf13/viper"
)

type Config struct {
	Database    DatabaseConfig
	DatabaseCNPJ DatabaseURLConfig
	DatabaseSaaS DatabaseURLConfig
	SaaS        SaasConfig
	Redis       RedisConfig
	ClickHouse  ClickHouseConfig
	Meilisearch MeilisearchConfig
	Server      ServerConfig
	Import      ImportConfig
	Cache       CacheConfig
	Logging     LoggingConfig
}

type DatabaseConfig struct {
	Host            string
	Port            int
	MigratePort     int
	User            string
	Password        string
	Name            string
	SSLMode         string
	MaxOpenConns    int
	MaxIdleConns    int
	ConnMaxLifetime int
	ConnMaxIdleTime int
}

type RedisConfig struct {
	URL      string
	Host     string
	Port     int
	Password string
	DB       int
	PoolSize int
}

type ClickHouseConfig struct {
	Host     string
	Port     int
	User     string
	Password string
	Database string
}

type MeilisearchConfig struct {
	Enabled               bool
	Host                  string
	Port                  int
	APIKey                string
	SelectiveActiveMatriz bool
}

type ServerConfig struct {
	Port                   int
	Prefork                bool
	ReadBufferSize         int
	WriteBufferSize        int
	RateLimitMax           int
	RateLimitWindowSeconds int
}

type ImportConfig struct {
	Workers        int
	ParseWorkers   int
	CopyWorkers    int
	BatchSize      int
	ReadBufferSize int
	DataPath       string
}

type CacheConfig struct {
	Enabled       bool
	TTL           int
	TTLCNPJ       int
	TTLSearch     int
	TTLAnalytics  int
	TTLLookup     int
	L1Enabled     bool
	L1MaxCostMB   int
	L1NumCounters int64
	L1BufferItems int64
}

type LoggingConfig struct {
	Level  string
	Format string
}

var AppConfig *Config

func Load() error {
	viper.AutomaticEnv()

	if path := os.Getenv("CONFIG_FILE"); path != "" {
		viper.SetConfigFile(path)
	} else {
		viper.SetConfigName("config")
		viper.SetConfigType("yaml")
		viper.AddConfigPath("./config")
		viper.AddConfigPath(".")
	}

	setDefaults()

	if err := viper.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			return fmt.Errorf("error reading config file: %w", err)
		}
	}

	applySaasRateLimitDefaults()

	saasCfg, dbCNPJ, dbSaaS := loadSaasFromViper()

	AppConfig = &Config{
		SaaS:         saasCfg,
		DatabaseCNPJ: dbCNPJ,
		DatabaseSaaS: dbSaaS,
		Database: DatabaseConfig{
			Host:            viper.GetString("database.host"),
			Port:            viper.GetInt("database.port"),
			MigratePort:     viper.GetInt("database.migrate_port"),
			User:            viper.GetString("database.user"),
			Password:        viper.GetString("database.password"),
			Name:            viper.GetString("database.name"),
			SSLMode:         viper.GetString("database.sslmode"),
			MaxOpenConns:    viper.GetInt("database.max_open_conns"),
			MaxIdleConns:    viper.GetInt("database.max_idle_conns"),
			ConnMaxLifetime: viper.GetInt("database.conn_max_lifetime"),
			ConnMaxIdleTime: viper.GetInt("database.conn_max_idle_time"),
		},
		Redis: loadRedisConfig(),
		ClickHouse: ClickHouseConfig{
			Host:     viper.GetString("clickhouse.host"),
			Port:     viper.GetInt("clickhouse.port"),
			User:     viper.GetString("clickhouse.user"),
			Password: viper.GetString("clickhouse.password"),
			Database: viper.GetString("clickhouse.database"),
		},
		Meilisearch: MeilisearchConfig{
			Enabled:               viper.GetBool("meilisearch.enabled"),
			Host:                  viper.GetString("meilisearch.host"),
			Port:                  viper.GetInt("meilisearch.port"),
			APIKey:                viper.GetString("meilisearch.api_key"),
			SelectiveActiveMatriz: viper.GetBool("meilisearch.selective_active_matriz"),
		},
		Server: ServerConfig{
			Port:                   viper.GetInt("server.port"),
			Prefork:                viper.GetBool("server.prefork"),
			ReadBufferSize:         viper.GetInt("server.read_buffer_size"),
			WriteBufferSize:        viper.GetInt("server.write_buffer_size"),
			RateLimitMax:           resolveRateLimitMax(),
			RateLimitWindowSeconds: resolveRateLimitWindow(),
		},
		Import: ImportConfig{
			Workers:        viper.GetInt("import.workers"),
			ParseWorkers:   viper.GetInt("import.parse_workers"),
			CopyWorkers:    viper.GetInt("import.copy_workers"),
			BatchSize:      viper.GetInt("import.batch_size"),
			ReadBufferSize: viper.GetInt("import.read_buffer_size"),
			DataPath:       viper.GetString("import.data_path"),
		},
		Cache: CacheConfig{
			Enabled:       viper.GetBool("cache.enabled"),
			TTL:           viper.GetInt("cache.ttl"),
			TTLCNPJ:       viper.GetInt("cache.ttl_cnpj"),
			TTLSearch:     viper.GetInt("cache.ttl_search"),
			TTLAnalytics:  viper.GetInt("cache.ttl_analytics"),
			TTLLookup:     viper.GetInt("cache.ttl_lookup"),
			L1Enabled:     viper.GetBool("cache.l1_enabled"),
			L1MaxCostMB:   viper.GetInt("cache.l1_max_cost_mb"),
			L1NumCounters: viper.GetInt64("cache.l1_num_counters"),
			L1BufferItems: viper.GetInt64("cache.l1_buffer_items"),
		},
		Logging: LoggingConfig{
			Level:  viper.GetString("logging.level"),
			Format: viper.GetString("logging.format"),
		},
	}

	// Adjust workers based on CPU count if not explicitly set
	if AppConfig.Import.Workers == 0 {
		AppConfig.Import.Workers = runtime.NumCPU() * 2
	}
	if AppConfig.Import.ParseWorkers == 0 {
		AppConfig.Import.ParseWorkers = runtime.NumCPU()
	}
	if AppConfig.Import.CopyWorkers == 0 {
		AppConfig.Import.CopyWorkers = max(1, runtime.NumCPU()/2)
	}

	// Adjust database connection pool based on CPU count
	if AppConfig.Database.MaxOpenConns == 0 {
		AppConfig.Database.MaxOpenConns = runtime.NumCPU() * 4
	}
	if AppConfig.Database.MaxIdleConns == 0 {
		AppConfig.Database.MaxIdleConns = runtime.NumCPU() * 2
	}

	return nil
}

func loadRedisConfig() RedisConfig {
	cfg := RedisConfig{
		URL:      firstNonEmpty(os.Getenv("REDIS_URL"), viper.GetString("redis.url")),
		Host:     viper.GetString("redis.host"),
		Port:     viper.GetInt("redis.port"),
		Password: viper.GetString("redis.password"),
		DB:       viper.GetInt("redis.db"),
		PoolSize: viper.GetInt("redis.pool_size"),
	}
	applyRedisURLDefaults(&cfg)
	return cfg
}

func applyRedisURLDefaults(cfg *RedisConfig) {
	if cfg.URL == "" {
		return
	}
	host, port, password, db, err := parseRedisURL(cfg.URL)
	if err != nil {
		return
	}
	cfg.Host = host
	cfg.Port = port
	cfg.Password = password
	cfg.DB = db
}

func setDefaults() {
	viper.SetDefault("database.host", "localhost")
	viper.SetDefault("database.port", 6432)
	viper.SetDefault("database.migrate_port", 5434)
	viper.SetDefault("database.user", "receita_user")
	viper.SetDefault("database.password", "receita_password")
	viper.SetDefault("database.name", "receita_db")
	viper.SetDefault("database.sslmode", "disable")
	viper.SetDefault("database.max_open_conns", 20)
	viper.SetDefault("database.max_idle_conns", 5)
	viper.SetDefault("database.conn_max_lifetime", 3600)
	viper.SetDefault("database.conn_max_idle_time", 30)

	viper.SetDefault("redis.host", "localhost")
	viper.SetDefault("redis.port", 6379)
	viper.SetDefault("redis.password", "")
	viper.SetDefault("redis.db", 0)
	viper.SetDefault("redis.pool_size", 10)

	viper.SetDefault("clickhouse.host", "localhost")
	viper.SetDefault("clickhouse.port", 9000)
	viper.SetDefault("clickhouse.user", "receita_user")
	viper.SetDefault("clickhouse.password", "receita_password")
	viper.SetDefault("clickhouse.database", "receita_analytics")

	viper.SetDefault("meilisearch.enabled", false)
	viper.SetDefault("meilisearch.host", "localhost")
	viper.SetDefault("meilisearch.port", 7700)
	viper.SetDefault("meilisearch.api_key", "dev_master_key_change_me")
	viper.SetDefault("meilisearch.selective_active_matriz", true)

	viper.SetDefault("server.port", 8080)
	viper.SetDefault("server.prefork", false)
	viper.SetDefault("server.read_buffer_size", 16384)
	viper.SetDefault("server.write_buffer_size", 4096)
	viper.SetDefault("server.rate_limit_max", 6000)
	viper.SetDefault("server.rate_limit_window_seconds", 60)

	viper.SetDefault("import.workers", runtime.NumCPU()*2) // Optimized: 2x CPU cores for I/O bound operations
	viper.SetDefault("import.parse_workers", runtime.NumCPU())
	viper.SetDefault("import.copy_workers", max(1, runtime.NumCPU()/2))
	viper.SetDefault("import.batch_size", 250000)        // Optimized batch size for 32GB RAM and PostgreSQL 18.4
	viper.SetDefault("import.read_buffer_size", 4194304) // 4MB buffer for faster CSV reading
	viper.SetDefault("import.data_path", "./data")

	viper.SetDefault("cache.enabled", true)
	viper.SetDefault("cache.ttl", 300)
	viper.SetDefault("cache.ttl_cnpj", 86400)
	viper.SetDefault("cache.ttl_search", 300)
	viper.SetDefault("cache.ttl_analytics", 3600)
	viper.SetDefault("cache.ttl_lookup", 900)
	viper.SetDefault("cache.l1_enabled", true)
	viper.SetDefault("cache.l1_max_cost_mb", 256)
	viper.SetDefault("cache.l1_num_counters", 10000000)
	viper.SetDefault("cache.l1_buffer_items", 64)

	viper.SetDefault("logging.level", "info")
	viper.SetDefault("logging.format", "json")

	viper.SetDefault("saas.enabled", false)
	viper.SetDefault("saas.public_api_only", false)
	viper.SetDefault("saas.admin_enabled", false)
	viper.SetDefault("saas.admin_jwt_ttl_minutes", 15)
	viper.SetDefault("saas.admin_refresh_ttl_days", 30)
	viper.SetDefault("saas.mfa_required", true)
	viper.SetDefault("saas.mfa_totp_issuer", "OpenCNPJ-Admin")
	viper.SetDefault("saas.default_client_rate_per_min", 60)
	viper.SetDefault("saas.default_monthly_quota", 0)
	viper.SetDefault("saas.docs_enabled", false)
	viper.SetDefault("saas.docs_public_url", "https://github.com/AlexandreZanata/BUSCA-CNPJ-2026/blob/main/docs/api/QUICKSTART.md")

	// Load .env file if exists - use godotenv or manual loading
	if _, err := os.Stat(".env"); err == nil {
		// Read .env file manually
		envFile, err := os.Open(".env")
		if err == nil {
			defer envFile.Close()
			scanner := bufio.NewScanner(envFile)
			for scanner.Scan() {
				line := strings.TrimSpace(scanner.Text())
				if line == "" || strings.HasPrefix(line, "#") {
					continue
				}
				parts := strings.SplitN(line, "=", 2)
				if len(parts) == 2 {
					key := strings.TrimSpace(parts[0])
					value := strings.TrimSpace(parts[1])
					// Remove quotes if present
					if value != "" && (value[0] == '"' || value[0] == '\'') {
						value = value[1 : len(value)-1]
					}
					if setErr := os.Setenv(key, value); setErr != nil {
						continue
					}
				}
			}
		}
	}
}

func GetDSN() string {
	return buildDSN(AppConfig.Database.Port)
}

func GetMigrateDSN() string {
	port := AppConfig.Database.Port
	if AppConfig.Database.MigratePort > 0 {
		port = AppConfig.Database.MigratePort
	}
	return buildDSN(port)
}

func buildDSN(port int) string {
	return fmt.Sprintf("host=%s port=%d user=%s password=%s dbname=%s sslmode=%s",
		AppConfig.Database.Host,
		port,
		AppConfig.Database.User,
		AppConfig.Database.Password,
		AppConfig.Database.Name,
		AppConfig.Database.SSLMode,
	)
}

func GetRedisAddr() string {
	return fmt.Sprintf("%s:%d", AppConfig.Redis.Host, AppConfig.Redis.Port)
}

func GetClickHouseDSN() string {
	return fmt.Sprintf("clickhouse://%s:%s@%s:%d/%s",
		AppConfig.ClickHouse.User,
		AppConfig.ClickHouse.Password,
		AppConfig.ClickHouse.Host,
		AppConfig.ClickHouse.Port,
		AppConfig.ClickHouse.Database,
	)
}
