package services

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"time"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
)

type CacheService struct {
	enabled bool
	ttl     cacheTTLProfile
}

var errCacheDisabled = errors.New("cache disabled")

func NewCacheService() *CacheService {
	return &CacheService{
		enabled: config.AppConfig.Cache.Enabled,
		ttl:     newCacheTTLProfile(),
	}
}

// GetOrSet retrieves a value from cache or executes fn and stores the result.
func (s *CacheService) GetOrSet(ctx context.Context, key string, fn func() (interface{}, error)) (interface{}, error) {
	if !s.enabled {
		return fn()
	}

	val, err := database.RedisClient.Get(ctx, key).Result()
	if err == nil {
		var result interface{}
		if unmarshalErr := unmarshalCacheValue([]byte(val), &result); unmarshalErr == nil {
			recordCacheHit(key)
			return result, nil
		}
	}

	recordCacheMiss(key)

	result, err := fn()
	if err != nil {
		return nil, err
	}

	if setErr := s.setWithTTL(ctx, key, result, s.ttl.forKey(key)); setErr != nil {
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
	return database.RedisClient.Set(ctx, key, data, ttl).Err()
}

// Get retrieves a value from cache.
func (s *CacheService) Get(ctx context.Context, key string) (interface{}, error) {
	if !s.enabled {
		return nil, errCacheDisabled
	}

	val, err := database.RedisClient.Get(ctx, key).Result()
	if err != nil {
		return nil, err
	}

	var result interface{}
	if err := unmarshalCacheValue([]byte(val), &result); err != nil {
		return nil, fmt.Errorf("failed to unmarshal cache value: %w", err)
	}

	return result, nil
}

// Delete removes a key from cache.
func (s *CacheService) Delete(ctx context.Context, key string) error {
	if !s.enabled {
		return nil
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
