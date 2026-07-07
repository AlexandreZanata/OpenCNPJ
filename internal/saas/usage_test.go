package saas_test

import (
	"context"
	"testing"
	"time"

	"github.com/alicebob/miniredis/v2"
	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/saas"
)

func TestUsageTrackerIncrementsRedis(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()

	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	tracker := saas.NewUsageTracker(rdb, nil, time.Hour)
	tracker.Start(context.Background())
	defer tracker.Stop()

	id := uuid.New()
	tracker.RecordRequest(id)
	tracker.RecordCNPJLookup(id)
	time.Sleep(50 * time.Millisecond)

	day := time.Now().UTC().Format("2006-01-02")
	key := "usage:client:" + id.String() + ":day:" + day
	reqs, err := rdb.HGet(context.Background(), key, "request_count").Int64()
	if err != nil {
		t.Fatal(err)
	}
	if reqs != 1 {
		t.Fatalf("request_count = %d, want 1", reqs)
	}
	cnpj, err := rdb.HGet(context.Background(), key, "cnpj_lookup_count").Int64()
	if err != nil {
		t.Fatal(err)
	}
	if cnpj != 1 {
		t.Fatalf("cnpj_lookup_count = %d, want 1", cnpj)
	}
}

func TestRedisRateLimiterBlocksOverLimit(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()

	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	lim := saas.NewRedisRateLimiter(rdb)
	id := uuid.New()
	ctx := context.Background()

	ok, err := lim.Allow(ctx, id, 2)
	if err != nil || !ok {
		t.Fatalf("first allow: ok=%v err=%v", ok, err)
	}
	ok, err = lim.Allow(ctx, id, 2)
	if err != nil || !ok {
		t.Fatalf("second allow: ok=%v err=%v", ok, err)
	}
	ok, err = lim.Allow(ctx, id, 2)
	if err != nil {
		t.Fatal(err)
	}
	if ok {
		t.Fatal("expected rate limit block on third request")
	}
}
