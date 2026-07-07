package saas

import (
	"context"
	"log"
	"time"

	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/database"
)

// WireDeps builds SaaS middleware dependencies when saas.enabled is true.
func WireDeps(ctx context.Context) (*Deps, func(), error) {
	if err := database.InitSaaSPgx(); err != nil {
		return nil, nil, err
	}
	queries := saasdb.New(database.SaaSPool)
	var rateLimit ClientRateLimiter = NoopRateLimiter{}
	var usage UsageRecorder = NoopUsageRecorder{}
	if database.RedisClient != nil {
		rateLimit = NewRedisRateLimiter(database.RedisClient)
		tracker := NewUsageTracker(database.RedisClient, queries, 5*time.Minute)
		tracker.Start(ctx)
		usage = tracker
	} else {
		log.Println("Warning: Redis unavailable — SaaS usage/rate limits degraded")
	}
	deps := &Deps{
		Auth:      NewKeyStore(queries),
		RateLimit: rateLimit,
		Usage:     usage,
	}
	cleanup := func() {
		if t, ok := usage.(*UsageTracker); ok {
			t.Stop()
		}
		database.CloseSaaSPgx()
	}
	return deps, cleanup, nil
}
