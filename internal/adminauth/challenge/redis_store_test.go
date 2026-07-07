package challenge

import (
	"context"
	"testing"
	"time"

	"github.com/alicebob/miniredis/v2"
	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"
)

func TestCreateAndConsume(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	store := NewStore(rdb, 300)
	ctx := context.Background()
	adminID := uuid.New()
	id, err := store.Create(ctx, adminID, "admin@test.local")
	if err != nil {
		t.Fatal(err)
	}
	got, err := store.Consume(ctx, id)
	if err != nil {
		t.Fatal(err)
	}
	if got.AdminID != adminID || got.Email != "admin@test.local" {
		t.Fatalf("payload mismatch: %+v", got)
	}
	if _, err := store.Consume(ctx, id); err == nil {
		t.Fatal("second consume should fail")
	}
}

func TestChallengeExpires(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	store := NewStore(rdb, 1)
	ctx := context.Background()
	id, err := store.Create(ctx, uuid.New(), "a@b.c")
	if err != nil {
		t.Fatal(err)
	}
	mr.FastForward(2 * time.Second)
	if _, err := store.Consume(ctx, id); err == nil {
		t.Fatal("expired challenge should fail")
	}
}
