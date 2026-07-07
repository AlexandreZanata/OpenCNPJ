package saas

import (
	"context"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"
)

// ClientRateLimiter enforces per-client request limits via Redis.
type ClientRateLimiter interface {
	Allow(ctx context.Context, clientID uuid.UUID, limitPerMin int) (bool, error)
}

// RedisRateLimiter tracks per-minute counters in Redis.
type RedisRateLimiter struct {
	rdb *redis.Client
}

// NewRedisRateLimiter returns a Redis-backed per-client rate limiter.
func NewRedisRateLimiter(rdb *redis.Client) *RedisRateLimiter {
	return &RedisRateLimiter{rdb: rdb}
}

// Allow increments the minute bucket and reports whether the request is allowed.
func (r *RedisRateLimiter) Allow(ctx context.Context, clientID uuid.UUID, limitPerMin int) (bool, error) {
	if r.rdb == nil || limitPerMin <= 0 {
		return true, nil
	}
	minute := time.Now().Unix() / 60
	key := fmt.Sprintf("ratelimit:client:%s:minute:%d", clientID, minute)
	count, err := r.rdb.Incr(ctx, key).Result()
	if err != nil {
		return false, fmt.Errorf("redis incr rate: %w", err)
	}
	if count == 1 {
		_ = r.rdb.Expire(ctx, key, time.Minute).Err()
	}
	return count <= int64(limitPerMin), nil
}

// NoopRateLimiter always allows requests (tests without Redis).
type NoopRateLimiter struct{}

func (NoopRateLimiter) Allow(context.Context, uuid.UUID, int) (bool, error) {
	return true, nil
}
