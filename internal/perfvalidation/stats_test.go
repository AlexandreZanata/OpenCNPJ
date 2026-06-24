package perfvalidation

import "testing"

func TestRedisHitRate(t *testing.T) {
	if got := RedisHitRate(9, 6); got < 59.9 || got > 60.1 {
		t.Fatalf("hit rate = %v, want ~60", got)
	}
	if RedisHitRate(0, 0) != 0 {
		t.Fatal("expected zero hit rate for empty stats")
	}
}

func TestMeetsHitRateTarget(t *testing.T) {
	if !MeetsHitRateTarget(9, 6, 40) {
		t.Fatal("60% should meet 40% target")
	}
	if MeetsHitRateTarget(1, 9, 40) {
		t.Fatal("10% should not meet 40% target")
	}
}
