package services

import (
	"context"
	"encoding/json"
	"fmt"

	"busca-cnpj-2026/internal/database"
)

// GetOrSetJSON retrieves a typed value from cache or executes fn and stores the result.
func GetOrSetJSON[T any](
	ctx context.Context,
	s *CacheService,
	key string,
	fn func() (T, error),
) (T, error) {
	var zero T
	if !s.enabled {
		return fn()
	}

	val, err := database.RedisClient.Get(ctx, key).Result()
	if err == nil {
		var result T
		if unmarshalErr := json.Unmarshal([]byte(val), &result); unmarshalErr == nil {
			recordCacheHit(key)
			return result, nil
		}
		_ = s.Delete(ctx, key)
	}

	recordCacheMiss(key)

	result, err := fn()
	if err != nil {
		return zero, err
	}

	if setErr := s.Set(ctx, key, result); setErr != nil {
		fmt.Printf("Failed to set cache key %s: %v\n", key, setErr)
	}

	return result, nil
}
