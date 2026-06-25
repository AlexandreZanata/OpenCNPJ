package l1

import (
	"testing"
	"time"
)

func TestCacheSetGetTTL(t *testing.T) {
	c, err := New(Config{MaxCostMB: 1, NumCounters: 1000, BufferItems: 8})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer c.Close()

	payload := []byte{0x01, 0x02, 0x03}
	c.SetWithTTL("estabelecimento:cnpj:v2:123", payload, time.Minute)
	c.Wait()

	got, ok := c.Get("estabelecimento:cnpj:v2:123")
	if !ok {
		t.Fatal("expected L1 hit")
	}
	if len(got) != len(payload) {
		t.Fatalf("len = %d, want %d", len(got), len(payload))
	}
}

func TestCacheDelete(t *testing.T) {
	c, err := New(Config{MaxCostMB: 1, NumCounters: 1000, BufferItems: 8})
	if err != nil {
		t.Fatalf("New: %v", err)
	}
	defer c.Close()

	c.SetWithTTL("k", []byte("v"), time.Minute)
	c.Wait()
	c.Delete("k")
	c.Wait()

	if _, ok := c.Get("k"); ok {
		t.Fatal("expected key deleted")
	}
}

func TestCacheNilSafe(t *testing.T) {
	var c *Cache
	if _, ok := c.Get("x"); ok {
		t.Fatal("nil cache must miss")
	}
	c.SetWithTTL("x", []byte("y"), time.Second)
	c.Delete("x")
	c.Close()
	c.Wait()
}
