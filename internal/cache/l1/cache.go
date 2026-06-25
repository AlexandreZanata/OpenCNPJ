package l1

import (
	"fmt"
	"time"

	"github.com/dgraph-io/ristretto/v2"
)

// Config holds Ristretto L1 cache sizing (plan 02 Phase 3).
type Config struct {
	MaxCostMB   int
	NumCounters int64
	BufferItems int64
}

// Cache is an in-process byte cache (msgpack payloads) above Redis L2.
type Cache struct {
	inner *ristretto.Cache[string, []byte]
}

// New builds a Ristretto cache from Config.
func New(cfg Config) (*Cache, error) {
	if cfg.MaxCostMB <= 0 {
		cfg.MaxCostMB = 256
	}
	if cfg.NumCounters <= 0 {
		cfg.NumCounters = 10_000_000
	}
	if cfg.BufferItems <= 0 {
		cfg.BufferItems = 64
	}

	inner, err := ristretto.NewCache(&ristretto.Config[string, []byte]{
		NumCounters: cfg.NumCounters,
		MaxCost:     int64(cfg.MaxCostMB) << 20,
		BufferItems: cfg.BufferItems,
	})
	if err != nil {
		return nil, fmt.Errorf("ristretto new cache: %w", err)
	}
	return &Cache{inner: inner}, nil
}

// Get returns cached bytes and whether the key was present.
func (c *Cache) Get(key string) ([]byte, bool) {
	if c == nil || c.inner == nil {
		return nil, false
	}
	return c.inner.Get(key)
}

// SetWithTTL stores bytes; cost equals payload length.
func (c *Cache) SetWithTTL(key string, value []byte, ttl time.Duration) {
	if c == nil || c.inner == nil || len(value) == 0 {
		return
	}
	cost := int64(len(value))
	if cost < 1 {
		cost = 1
	}
	_ = c.inner.SetWithTTL(key, value, cost, ttl)
}

// Delete removes a key from L1.
func (c *Cache) Delete(key string) {
	if c == nil || c.inner == nil {
		return
	}
	c.inner.Del(key)
}

// Close releases Ristretto resources.
func (c *Cache) Close() {
	if c == nil || c.inner == nil {
		return
	}
	c.inner.Close()
}

// Wait drains pending writes (tests and shutdown).
func (c *Cache) Wait() {
	if c == nil || c.inner == nil {
		return
	}
	c.inner.Wait()
}
