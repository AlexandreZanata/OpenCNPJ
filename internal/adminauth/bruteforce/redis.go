package bruteforce

import (
	"context"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"strings"
	"time"

	"github.com/redis/go-redis/v9"
)

const keyPrefix = "login_fail:email:"

// Guard tracks failed login attempts per email hash.
type Guard struct {
	rdb    redisCmd
	max    int
	lockTTL time.Duration
}

type redisCmd interface {
	Incr(ctx context.Context, key string) *redis.IntCmd
	Expire(ctx context.Context, key string, expiration time.Duration) *redis.BoolCmd
	Get(ctx context.Context, key string) *redis.StringCmd
	Del(ctx context.Context, keys ...string) *redis.IntCmd
}

// NewGuard returns a Redis-backed login brute-force guard.
func NewGuard(rdb redisCmd, maxFailures, lockoutMinutes int) *Guard {
	return &Guard{
		rdb:     rdb,
		max:     maxFailures,
		lockTTL: time.Duration(lockoutMinutes) * time.Minute,
	}
}

// IsLocked reports whether the email is temporarily locked.
func (g *Guard) IsLocked(ctx context.Context, email string) (bool, error) {
	n, err := g.rdb.Get(ctx, g.key(email)).Int()
	if err == redis.Nil {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("redis get login_fail: %w", err)
	}
	return n >= g.max, nil
}

// RecordFailure increments the failure counter and sets TTL on first hit.
func (g *Guard) RecordFailure(ctx context.Context, email string) error {
	k := g.key(email)
	n, err := g.rdb.Incr(ctx, k).Result()
	if err != nil {
		return fmt.Errorf("redis incr login_fail: %w", err)
	}
	if n == 1 {
		if err := g.rdb.Expire(ctx, k, g.lockTTL).Err(); err != nil {
			return err
		}
	}
	return nil
}

// ClearFailures resets the counter after successful password verification.
func (g *Guard) ClearFailures(ctx context.Context, email string) error {
	return g.rdb.Del(ctx, g.key(email)).Err()
}

func (g *Guard) key(email string) string {
	norm := strings.ToLower(strings.TrimSpace(email))
	sum := sha256.Sum256([]byte(norm))
	return keyPrefix + hex.EncodeToString(sum[:])
}
