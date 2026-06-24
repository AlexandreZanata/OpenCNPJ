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
	ttl     time.Duration
}

var errCacheDisabled = errors.New("cache disabled")

func NewCacheService() *CacheService {
	ttl := time.Duration(config.AppConfig.Cache.TTL) * time.Second
	if ttl == 0 {
		ttl = 5 * time.Minute
	}

	return &CacheService{
		enabled: config.AppConfig.Cache.Enabled,
		ttl:     ttl,
	}
}

// GetOrSet retrieves a value from cache or executes fn and stores the result.
func (s *CacheService) GetOrSet(ctx context.Context, key string, fn func() (interface{}, error)) (interface{}, error) {
	if !s.enabled {
		return fn()
	}

	// Try to get from cache
	val, err := database.RedisClient.Get(ctx, key).Result()
	if err == nil {
		// Cache hit - deserialize
		var result interface{}
		if err := json.Unmarshal([]byte(val), &result); err == nil {
			recordCacheHit(key)
			return result, nil
		}
	}

	recordCacheMiss(key)

	// Cache miss - execute function
	result, err := fn()
	if err != nil {
		return nil, err
	}

	// Store in cache
	if err := s.Set(ctx, key, result); err != nil {
		// Log error but don't fail the request
		fmt.Printf("Failed to set cache key %s: %v\n", key, err)
	}

	return result, nil
}

// Set stores a value in cache.
func (s *CacheService) Set(ctx context.Context, key string, value interface{}) error {
	if !s.enabled {
		return nil
	}

	data, err := json.Marshal(value)
	if err != nil {
		return fmt.Errorf("failed to marshal cache value: %w", err)
	}

	return database.RedisClient.Set(ctx, key, data, s.ttl).Err()
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
	if err := json.Unmarshal([]byte(val), &result); err != nil {
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
	// Create a deterministic key from parameters
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
