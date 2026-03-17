package database

import (
	"context"
	"fmt"
	"time"

	"busca-cnpj-2026/internal/config"

	"github.com/ClickHouse/clickhouse-go/v2"
	"github.com/ClickHouse/clickhouse-go/v2/lib/driver"
)

var ClickHouseConn driver.Conn

func InitClickHouse() error {
	ctx := context.Background()
	
	conn, err := clickhouse.Open(&clickhouse.Options{
		Addr: []string{fmt.Sprintf("%s:%d", config.AppConfig.ClickHouse.Host, config.AppConfig.ClickHouse.Port)},
		Auth: clickhouse.Auth{
			Database: config.AppConfig.ClickHouse.Database,
			Username: config.AppConfig.ClickHouse.User,
			Password: config.AppConfig.ClickHouse.Password,
		},
		Settings: clickhouse.Settings{
			"max_execution_time": 60,
		},
		DialTimeout: 5 * time.Second,
		Compression: &clickhouse.Compression{
			Method: clickhouse.CompressionLZ4,
		},
	})
	if err != nil {
		return fmt.Errorf("failed to connect to clickhouse: %w", err)
	}

	if err := conn.Ping(ctx); err != nil {
		return fmt.Errorf("failed to ping clickhouse: %w", err)
	}

	ClickHouseConn = conn
	return nil
}

func CloseClickHouse() error {
	if ClickHouseConn != nil {
		return ClickHouseConn.Close()
	}
	return nil
}
