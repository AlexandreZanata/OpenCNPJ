package database

import (
	"context"
	"fmt"
	"time"

	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/config"
)

var RedisClient *redis.Client

func InitRedis() error {
	RedisClient = redis.NewClient(&redis.Options{
		Addr:         config.GetRedisAddr(),
		Password:     config.AppConfig.Redis.Password,
		DB:           config.AppConfig.Redis.DB,
		PoolSize:     config.AppConfig.Redis.PoolSize,
		MinIdleConns: 5,
		DialTimeout:  5 * time.Second,
		ReadTimeout:  3 * time.Second,
		WriteTimeout: 3 * time.Second,
	})

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	if err := RedisClient.Ping(ctx).Err(); err != nil {
		return fmt.Errorf("failed to connect to redis: %w", err)
	}

	return nil
}

func CloseRedis() error {
	if RedisClient != nil {
		return RedisClient.Close()
	}
	return nil
}
