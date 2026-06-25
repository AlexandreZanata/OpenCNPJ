package services

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"busca-cnpj-2026/internal/cache/l1"
	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"

	"github.com/redis/go-redis/v9"
)

type CacheService struct {
	enabled bool
	ttl     cacheTTLProfile
	l1      *l1.Cache
}

var errCacheDisabled = errors.New("cache disabled")

func NewCacheService() *CacheService {
	return &CacheService{
		enabled: config.AppConfig.Cache.Enabled,
		ttl:     newCacheTTLProfile(),
		l1:      newL1Cache(),
	}
}

// GetOrSet retrieves a value from cache or executes fn and stores the result.
func (s *CacheService) GetOrSet(ctx context.Context, key string, fn func() (interface{}, error)) (interface{}, error) {
	if !s.enabled {
		return fn()
	}

	val, hit, err := s.fetchCachedBytes(ctx, key)
	if hit {
		var result interface{}
		if unmarshalErr := unmarshalCacheValue(val, &result); unmarshalErr == nil {
			return result, nil
		}
		_ = s.Delete(ctx, key)
	} else if err != nil && !errors.Is(err, redis.Nil) {
		return nil, fmt.Errorf("cache fetch: %w", err)
	}

	recordCacheMiss(key)

	result, err := fn()
	if err != nil {
		return nil, err
	}

	if setErr := s.storeCachedValue(ctx, key, result); setErr != nil {
		fmt.Printf("Failed to set cache key %s: %v\n", key, setErr)
	}

	return result, nil
}

// Set stores a value in cache using the TTL derived from the key prefix.
func (s *CacheService) Set(ctx context.Context, key string, value interface{}) error {
	if !s.enabled {
		return nil
	}
	return s.setWithTTL(ctx, key, value, s.ttl.forKey(key))
}

func (s *CacheService) setWithTTL(ctx context.Context, key string, value interface{}, ttl time.Duration) error {
	data, err := marshalCacheValue(value)
	if err != nil {
		return fmt.Errorf("failed to marshal cache value: %w", err)
	}
	if err := database.RedisClient.Set(ctx, key, data, ttl).Err(); err != nil {
		return err
	}
	if s.l1 != nil {
		s.l1.SetWithTTL(key, data, ttl)
	}
	return nil
}

func (s *CacheService) storeCachedValue(ctx context.Context, key string, value interface{}) error {
	data, err := marshalCacheValue(value)
	if err != nil {
		return fmt.Errorf("failed to marshal cache value: %w", err)
	}
	return s.storeCachedBytes(ctx, key, data)
}

// Get retrieves a value from cache.
func (s *CacheService) Get(ctx context.Context, key string) (interface{}, error) {
	if !s.enabled {
		return nil, errCacheDisabled
	}

	val, hit, err := s.fetchCachedBytes(ctx, key)
	if hit {
		var result interface{}
		if unmarshalErr := unmarshalCacheValue(val, &result); unmarshalErr == nil {
			return result, nil
		}
		_ = s.Delete(ctx, key)
	}
	if err != nil && !errors.Is(err, redis.Nil) {
		return nil, fmt.Errorf("cache get: %w", err)
	}
	return nil, err
}

// Delete removes a key from cache.
func (s *CacheService) Delete(ctx context.Context, key string) error {
	if !s.enabled {
		return nil
	}
	if s.l1 != nil {
		s.l1.Delete(key)
	}

	return database.RedisClient.Del(ctx, key).Err()
}

// GenerateKey generates a cache key from a prefix and parameters.
func (s *CacheService) GenerateKey(prefix string, params map[string]interface{}) string {
	data, _ := json.Marshal(params)
	hash := sha256.Sum256(data)
	return fmt.Sprintf("%s:%x", prefix, hash)
}

// InvalidatePattern invalidates all keys matching a pattern.
func (s *CacheService) InvalidatePattern(ctx context.Context, pattern string) error {
	if !s.enabled {
		return nil
	}

	keys, err := database.RedisClient.Keys(ctx, pattern).Result()
	if err != nil {
		return err
	}

	if len(keys) > 0 {
		return database.RedisClient.Del(ctx, keys...).Err()
	}

	return nil
}
