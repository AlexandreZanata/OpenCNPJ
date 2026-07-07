package middleware_test

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/saas"
	"busca-cnpj-2026/internal/saas/middleware"
)

type stubAuth struct {
	client saas.AuthenticatedClient
	err    error
}

func (s stubAuth) Authenticate(context.Context, string) (saas.AuthenticatedClient, error) {
	return s.client, s.err
}

type stubRate struct {
	allow bool
	err   error
}

func (s stubRate) Allow(context.Context, uuid.UUID, int) (bool, error) {
	return s.allow, s.err
}

type stubUsage struct {
	requests int
}

func (s *stubUsage) RecordRequest(uuid.UUID) { s.requests++ }
func (s *stubUsage) RecordCNPJLookup(uuid.UUID) {}
func (s *stubUsage) MonthCount(context.Context, uuid.UUID) (int64, error) {
	return 0, nil
}
func (s *stubUsage) Flush(context.Context) error { return nil }
func (s *stubUsage) Start(context.Context)       {}
func (s *stubUsage) Stop()                       {}

type quotaUsage struct {
	count int64
	stubUsage
}

func (q *quotaUsage) MonthCount(context.Context, uuid.UUID) (int64, error) {
	return q.count, nil
}

func TestAPIKeyMissingHeader401(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{Auth: stubAuth{}}))
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	resp := mustRequest(t, app, "")
	if resp.StatusCode != fiber.StatusUnauthorized {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestAPIKeyInvalid401(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{
		Auth: stubAuth{err: saas.ErrInvalidKey},
	}))
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	resp := mustRequest(t, app, "ocnpj_live_bad")
	if resp.StatusCode != fiber.StatusUnauthorized {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestAPIKeyValid200(t *testing.T) {
	clientID := uuid.New()
	usage := &stubUsage{}
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{
		Auth: stubAuth{client: saas.AuthenticatedClient{
			ClientID:        clientID,
			RateLimitPerMin: 60,
			Status:          saas.ClientStatusActive,
		}},
		RateLimit: stubRate{allow: true},
		Usage:     usage,
	}))
	app.Get("/", func(c *fiber.Ctx) error {
		client, ok := middleware.ClientFromCtx(c)
		if !ok || client.ClientID != clientID {
			t.Fatal("client not in context")
		}
		return c.SendStatus(fiber.StatusOK)
	})

	resp := mustRequest(t, app, "ocnpj_live_"+repeatHex(32))
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	if usage.requests != 1 {
		t.Fatalf("requests = %d, want 1", usage.requests)
	}
}

func TestAPIKeyRateLimited429(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{
		Auth: stubAuth{client: saas.AuthenticatedClient{
			ClientID:        uuid.New(),
			RateLimitPerMin: 1,
			Status:          saas.ClientStatusActive,
		}},
		RateLimit: stubRate{allow: false},
		Usage:     &stubUsage{},
	}))
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	resp := mustRequest(t, app, "ocnpj_live_"+repeatHex(32))
	if resp.StatusCode != fiber.StatusTooManyRequests {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestAPIKeyExpired401(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{
		Auth: stubAuth{err: saas.ErrExpiredKey},
	}))
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	resp := mustRequest(t, app, "ocnpj_live_"+repeatHex(32))
	if resp.StatusCode != fiber.StatusUnauthorized {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestAPIKeyQuotaExceeded429(t *testing.T) {
	qu := &quotaUsage{count: 100}
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{
		Auth: stubAuth{client: saas.AuthenticatedClient{
			ClientID:     uuid.New(),
			MonthlyQuota: 100,
			Status:       saas.ClientStatusActive,
		}},
		RateLimit: stubRate{allow: true},
		Usage:     qu,
	}))
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	resp := mustRequest(t, app, "ocnpj_live_"+repeatHex(32))
	if resp.StatusCode != fiber.StatusTooManyRequests {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestAPIKeySuspended403(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.APIKey(&saas.Deps{
		Auth: stubAuth{err: saas.ErrSuspendedClient},
	}))
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	resp := mustRequest(t, app, "ocnpj_live_"+repeatHex(32))
	if resp.StatusCode != fiber.StatusForbidden {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func mustRequest(t *testing.T, app *fiber.App, apiKey string) *http.Response {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, "/", http.NoBody)
	if apiKey != "" {
		req.Header.Set(saas.HeaderAPIKey, apiKey)
	}
	resp, err := app.Test(req, 2000)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	return resp
}

func repeatHex(n int) string {
	const hex = "0123456789abcdef"
	out := make([]byte, n)
	for i := range out {
		out[i] = hex[i%16]
	}
	return string(out)
}
