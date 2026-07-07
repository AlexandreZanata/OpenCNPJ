package bruteforce

import (
	"context"
	"testing"

	"github.com/alicebob/miniredis/v2"
	"github.com/redis/go-redis/v9"
)

func TestGuardLockAfterMaxFailures(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	g := NewGuard(rdb, 3, 15)
	ctx := context.Background()
	email := "admin@test.local"
	for i := 0; i < 3; i++ {
		if err := g.RecordFailure(ctx, email); err != nil {
			t.Fatal(err)
		}
	}
	locked, err := g.IsLocked(ctx, email)
	if err != nil || !locked {
		t.Fatalf("expected locked, got locked=%v err=%v", locked, err)
	}
	if err := g.ClearFailures(ctx, email); err != nil {
		t.Fatal(err)
	}
	locked, err = g.IsLocked(ctx, email)
	if err != nil || locked {
		t.Fatalf("expected unlocked after clear")
	}
}
