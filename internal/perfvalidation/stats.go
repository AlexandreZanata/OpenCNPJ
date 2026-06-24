package perfvalidation

// RedisHitRate returns the percentage of cache hits over total operations.
func RedisHitRate(hits, misses int64) float64 {
	total := hits + misses
	if total == 0 {
		return 0
	}
	return float64(hits) / float64(total) * 100
}

// MeetsHitRateTarget reports whether observed hit rate meets the P1+ gate (default 40%).
func MeetsHitRateTarget(hits, misses int64, targetPercent float64) bool {
	if targetPercent <= 0 {
		targetPercent = 40
	}
	return RedisHitRate(hits, misses) >= targetPercent
}
