package saas

import (
	"context"
	"errors"
	"fmt"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"

	saasdb "busca-cnpj-2026/internal/db/saas"
)

// UsageRecorder records API usage asynchronously.
type UsageRecorder interface {
	RecordRequest(clientID uuid.UUID)
	RecordCNPJLookup(clientID uuid.UUID)
	MonthCount(ctx context.Context, clientID uuid.UUID) (int64, error)
	Flush(ctx context.Context) error
	Start(ctx context.Context)
	Stop()
}

// UsageTracker buffers Redis counters and flushes daily totals to Postgres.
type UsageTracker struct {
	rdb      *redis.Client
	queries  saasdb.Querier
	interval time.Duration
	ch       chan usageEvent
	stopCh   chan struct{}
	wg       sync.WaitGroup
}

type usageEvent struct {
	clientID uuid.UUID
	cnpj     bool
}

// NewUsageTracker builds a tracker with the given flush interval.
func NewUsageTracker(rdb *redis.Client, queries saasdb.Querier, interval time.Duration) *UsageTracker {
	if interval <= 0 {
		interval = 5 * time.Minute
	}
	return &UsageTracker{
		rdb:      rdb,
		queries:  queries,
		interval: interval,
		ch:       make(chan usageEvent, 1024),
		stopCh:   make(chan struct{}),
	}
}

// Start launches the async recorder and periodic flush goroutines.
func (t *UsageTracker) Start(ctx context.Context) {
	t.wg.Add(2)
	go t.consumeEvents(ctx)
	go t.flushLoop(ctx)
}

// Stop drains workers.
func (t *UsageTracker) Stop() {
	close(t.stopCh)
	t.wg.Wait()
}

// RecordRequest enqueues a generic API request counter increment.
func (t *UsageTracker) RecordRequest(clientID uuid.UUID) {
	t.enqueue(clientID, false)
}

// RecordCNPJLookup enqueues a CNPJ lookup counter increment.
func (t *UsageTracker) RecordCNPJLookup(clientID uuid.UUID) {
	t.enqueue(clientID, true)
}

func (t *UsageTracker) enqueue(clientID uuid.UUID, cnpj bool) {
	select {
	case t.ch <- usageEvent{clientID: clientID, cnpj: cnpj}:
	default:
		// Drop on overload — usage is best-effort.
	}
}

func (t *UsageTracker) consumeEvents(ctx context.Context) {
	defer t.wg.Done()
	for {
		select {
		case ev, ok := <-t.ch:
			if !ok {
				return
			}
			t.applyRedis(ctx, ev)
		case <-t.stopCh:
			return
		case <-ctx.Done():
			return
		}
	}
}

func (t *UsageTracker) applyRedis(ctx context.Context, ev usageEvent) {
	if t.rdb == nil {
		return
	}
	ctx, cancel := context.WithTimeout(ctx, 2*time.Second)
	defer cancel()
	day := time.Now().UTC().Format("2006-01-02")
	month := time.Now().UTC().Format("2006-01")
	dayKey := fmt.Sprintf("usage:client:%s:day:%s", ev.clientID, day)
	monthKey := fmt.Sprintf("usage:client:%s:month:%s", ev.clientID, month)
	pipe := t.rdb.Pipeline()
	if ev.cnpj {
		pipe.HIncrBy(ctx, dayKey, "cnpj_lookup_count", 1)
		pipe.HIncrBy(ctx, monthKey, "cnpj_lookup_count", 1)
	} else {
		pipe.HIncrBy(ctx, dayKey, "request_count", 1)
		pipe.HIncrBy(ctx, monthKey, "request_count", 1)
	}
	pipe.Expire(ctx, dayKey, 48*time.Hour)
	pipe.Expire(ctx, monthKey, 62*24*time.Hour)
	_, _ = pipe.Exec(ctx)
}

func (t *UsageTracker) flushLoop(ctx context.Context) {
	defer t.wg.Done()
	ticker := time.NewTicker(t.interval)
	defer ticker.Stop()
	for {
		select {
		case <-ticker.C:
			_ = t.Flush(ctx)
		case <-t.stopCh:
			flushCtx, cancel := context.WithTimeout(context.WithoutCancel(ctx), 10*time.Second)
			_ = t.Flush(flushCtx)
			cancel()
			return
		case <-ctx.Done():
			return
		}
	}
}

// Flush persists pending daily Redis counters to api_usage_daily.
func (t *UsageTracker) Flush(ctx context.Context) error {
	if t.rdb == nil || t.queries == nil {
		return nil
	}
	var cursor uint64
	pattern := "usage:client:*:day:*"
	for {
		keys, next, err := t.rdb.Scan(ctx, cursor, pattern, 50).Result()
		if err != nil {
			return fmt.Errorf("scan usage keys: %w", err)
		}
		for _, key := range keys {
			if err := t.flushKey(ctx, key); err != nil {
				return err
			}
		}
		cursor = next
		if cursor == 0 {
			break
		}
	}
	return nil
}

func (t *UsageTracker) flushKey(ctx context.Context, key string) error {
	parts := strings.Split(key, ":")
	if len(parts) != 5 {
		return nil
	}
	clientID, ok := parseUsageClientID(parts[2])
	if !ok {
		return nil
	}
	dateStr := parts[4]
	vals, err := t.rdb.HGetAll(ctx, key).Result()
	if err != nil {
		return fmt.Errorf("hgetall %s: %w", key, err)
	}
	reqs, _ := strconv.ParseInt(vals["request_count"], 10, 64)
	cnpj, _ := strconv.ParseInt(vals["cnpj_lookup_count"], 10, 64)
	if reqs == 0 && cnpj == 0 {
		return nil
	}
	day, ok := parseUsageDate(dateStr)
	if !ok {
		return nil
	}
	if err := t.queries.UpsertUsageDaily(ctx, saasdb.UpsertUsageDailyParams{
		ClientID:        pgUUID(clientID),
		Date:            pgDate(day),
		RequestCount:    reqs,
		CnpjLookupCount: cnpj,
	}); err != nil {
		return fmt.Errorf("upsert usage: %w", err)
	}
	_ = t.rdb.Del(ctx, key).Err()
	return nil
}

// MonthCount returns the current month request total from Redis.
func (t *UsageTracker) MonthCount(ctx context.Context, clientID uuid.UUID) (int64, error) {
	if t.rdb == nil {
		return 0, nil
	}
	month := time.Now().UTC().Format("2006-01")
	key := fmt.Sprintf("usage:client:%s:month:%s", clientID, month)
	val, err := t.rdb.HGet(ctx, key, "request_count").Result()
	if errors.Is(err, redis.Nil) {
		return 0, nil
	}
	if err != nil {
		return 0, err
	}
	return strconv.ParseInt(val, 10, 64)
}

func parseUsageClientID(part string) (uuid.UUID, bool) {
	id, err := uuid.Parse(part)
	if err != nil {
		return uuid.Nil, false
	}
	return id, true
}

func parseUsageDate(dateStr string) (time.Time, bool) {
	day, err := time.Parse("2006-01-02", dateStr)
	if err != nil {
		return time.Time{}, false
	}
	return day, true
}

// NoopUsageRecorder discards usage events (unit tests).
type NoopUsageRecorder struct{}

func (NoopUsageRecorder) RecordRequest(uuid.UUID)    {}
func (NoopUsageRecorder) RecordCNPJLookup(uuid.UUID) {}
func (NoopUsageRecorder) MonthCount(context.Context, uuid.UUID) (int64, error) {
	return 0, nil
}
func (NoopUsageRecorder) Flush(context.Context) error { return nil }
func (NoopUsageRecorder) Start(context.Context)       {}
func (NoopUsageRecorder) Stop()                       {}
