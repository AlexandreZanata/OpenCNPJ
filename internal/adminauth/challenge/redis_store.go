package challenge

import (
	"context"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"
)

const keyPrefix = "challenge:"

// Payload stored in Redis between login and MFA verify.
type Payload struct {
	AdminID uuid.UUID `json:"adminId"`
	Email   string    `json:"email"`
}

// Store persists MFA login challenges in Redis.
type Store struct {
	rdb redisCmd
	ttl time.Duration
}

type redisCmd interface {
	Set(ctx context.Context, key string, value any, expiration time.Duration) *redis.StatusCmd
	GetDel(ctx context.Context, key string) *redis.StringCmd
}

// NewStore returns a Redis-backed MFA challenge store.
func NewStore(rdb redisCmd, ttlSeconds int) *Store {
	return &Store{rdb: rdb, ttl: time.Duration(ttlSeconds) * time.Second}
}

// Create stores a challenge and returns its ID.
func (s *Store) Create(ctx context.Context, adminID uuid.UUID, email string) (uuid.UUID, error) {
	id := uuid.New()
	body, err := json.Marshal(Payload{AdminID: adminID, Email: email})
	if err != nil {
		return uuid.Nil, err
	}
	key := keyPrefix + id.String()
	if err := s.rdb.Set(ctx, key, body, s.ttl).Err(); err != nil {
		return uuid.Nil, fmt.Errorf("redis set challenge: %w", err)
	}
	return id, nil
}

// Consume loads and deletes a challenge atomically.
func (s *Store) Consume(ctx context.Context, id uuid.UUID) (Payload, error) {
	key := keyPrefix + id.String()
	raw, err := s.rdb.GetDel(ctx, key).Result()
	if err == redis.Nil {
		return Payload{}, fmt.Errorf("challenge not found")
	}
	if err != nil {
		return Payload{}, fmt.Errorf("redis getdel challenge: %w", err)
	}
	var p Payload
	if err := json.Unmarshal([]byte(raw), &p); err != nil {
		return Payload{}, err
	}
	return p, nil
}
