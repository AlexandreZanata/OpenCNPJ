package services

import (
	"context"
	"errors"
	"fmt"

	"github.com/redis/go-redis/v9"
)

// GetOrSetJSON retrieves a typed value from L1 → Redis or executes fn and stores the result.
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

	val, hit, err := s.fetchCachedBytes(ctx, key)
	if hit {
		var result T
		if unmarshalErr := unmarshalCacheValue(val, &result); unmarshalErr == nil {
			return result, nil
		}
		_ = s.Delete(ctx, key)
	} else if err != nil && !errors.Is(err, redis.Nil) {
		// Soft-fallback: Redis outage must not take down CNPJ lookups.
		recordCacheMiss(key)
		return fn()
	}

	recordCacheMiss(key)

	result, err := fn()
	if err != nil {
		return zero, err
	}

	if setErr := s.storeCachedValue(ctx, key, result); setErr != nil {
		fmt.Printf("Failed to set cache key %s: %v\n", key, setErr)
	}

	return result, nil
}
