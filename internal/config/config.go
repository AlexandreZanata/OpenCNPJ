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
	Database   DatabaseConfig
	Redis      RedisConfig
	ClickHouse ClickHouseConfig
	Server     ServerConfig
	Import     ImportConfig
	Cache      CacheConfig
	Logging    LoggingConfig
}

type DatabaseConfig struct {
	Host            string
	Port            int
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

type ServerConfig struct {
	Port                    int
	Prefork                 bool
	ReadBufferSize          int
	WriteBufferSize         int
	RateLimitMax            int
	RateLimitWindowSeconds  int
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
	Enabled bool
	TTL     int
}

type LoggingConfig struct {
	Level  string
	Format string
}

var AppConfig *Config

func Load() error {
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")
	viper.AddConfigPath("./config")
	viper.AddConfigPath(".")

	// Enable environment variables
	viper.AutomaticEnv()

	// Set defaults
	setDefaults()

	// Read config file
	if err := viper.ReadInConfig(); err != nil {
		if _, ok := err.(viper.ConfigFileNotFoundError); !ok {
			return fmt.Errorf("error reading config file: %w", err)
		}
	}

	AppConfig = &Config{
		Database: DatabaseConfig{
			Host:            viper.GetString("database.host"),
			Port:            viper.GetInt("database.port"),
			User:            viper.GetString("database.user"),
			Password:        viper.GetString("database.password"),
			Name:            viper.GetString("database.name"),
			SSLMode:         viper.GetString("database.sslmode"),
			MaxOpenConns:    viper.GetInt("database.max_open_conns"),
			MaxIdleConns:    viper.GetInt("database.max_idle_conns"),
			ConnMaxLifetime: viper.GetInt("database.conn_max_lifetime"),
			ConnMaxIdleTime: viper.GetInt("database.conn_max_idle_time"),
		},
		Redis: RedisConfig{
			Host:     viper.GetString("redis.host"),
			Port:     viper.GetInt("redis.port"),
			Password: viper.GetString("redis.password"),
			DB:       viper.GetInt("redis.db"),
			PoolSize: viper.GetInt("redis.pool_size"),
		},
		ClickHouse: ClickHouseConfig{
			Host:     viper.GetString("clickhouse.host"),
			Port:     viper.GetInt("clickhouse.port"),
			User:     viper.GetString("clickhouse.user"),
			Password: viper.GetString("clickhouse.password"),
			Database: viper.GetString("clickhouse.database"),
		},
		Server: ServerConfig{
			Port:                   viper.GetInt("server.port"),
			Prefork:                viper.GetBool("server.prefork"),
			ReadBufferSize:         viper.GetInt("server.read_buffer_size"),
			WriteBufferSize:        viper.GetInt("server.write_buffer_size"),
			RateLimitMax:           viper.GetInt("server.rate_limit_max"),
			RateLimitWindowSeconds: viper.GetInt("server.rate_limit_window_seconds"),
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
			Enabled: viper.GetBool("cache.enabled"),
			TTL:     viper.GetInt("cache.ttl"),
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

func setDefaults() {
	viper.SetDefault("database.host", "localhost")
	viper.SetDefault("database.port", 5432)
	viper.SetDefault("database.user", "receita_user")
	viper.SetDefault("database.password", "receita_password")
	viper.SetDefault("database.name", "receita_db")
	viper.SetDefault("database.sslmode", "disable")
	viper.SetDefault("database.max_open_conns", runtime.NumCPU()*4)
	viper.SetDefault("database.max_idle_conns", runtime.NumCPU()*2)
	viper.SetDefault("database.conn_max_lifetime", 3600)
	viper.SetDefault("database.conn_max_idle_time", 600)

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

	viper.SetDefault("logging.level", "info")
	viper.SetDefault("logging.format", "json")

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
	return fmt.Sprintf("host=%s port=%d user=%s password=%s dbname=%s sslmode=%s",
		AppConfig.Database.Host,
		AppConfig.Database.Port,
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
