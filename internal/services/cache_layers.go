package services

import (
	"context"
	"fmt"

	"busca-cnpj-2026/internal/cache/l1"
	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"

	"github.com/redis/go-redis/v9"
)

func newL1Cache() *l1.Cache {
	if !config.AppConfig.Cache.L1Enabled {
		return nil
	}
	c, err := l1.New(l1.Config{
		MaxCostMB:   config.AppConfig.Cache.L1MaxCostMB,
		NumCounters: config.AppConfig.Cache.L1NumCounters,
		BufferItems: config.AppConfig.Cache.L1BufferItems,
	})
	if err != nil {
		fmt.Printf("L1 cache init failed (continuing with Redis only): %v\n", err)
		return nil
	}
	return c
}

// fetchCachedBytes checks L1 then Redis. hit=true when bytes are a valid cache payload.
func (s *CacheService) fetchCachedBytes(ctx context.Context, key string) ([]byte, bool, error) {
	if s.l1 != nil {
		if data, ok := s.l1.Get(key); ok {
			recordL1CacheHit(key)
			return data, true, nil
		}
		recordL1CacheMiss(key)
	}

	val, err := database.RedisClient.Get(ctx, key).Result()
	if err != nil {
		if err == redis.Nil {
			return nil, false, redis.Nil
		}
		return nil, false, err
	}
	recordCacheHit(key)
	return []byte(val), true, nil
}

func (s *CacheService) storeCachedBytes(ctx context.Context, key string, data []byte) error {
	ttl := s.ttl.forKey(key)
	if err := database.RedisClient.Set(ctx, key, data, ttl).Err(); err != nil {
		return err
	}
	if s.l1 != nil {
		s.l1.SetWithTTL(key, data, ttl)
	}
	return nil
}
