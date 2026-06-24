package middleware

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/config"
)

func TestRateLimiterUsesConfig(t *testing.T) {
	config.AppConfig = &config.Config{
		Server: config.ServerConfig{
			RateLimitMax:           500,
			RateLimitWindowSeconds: 30,
		},
	}

	handler := RateLimiter()
	if handler == nil {
		t.Fatal("RateLimiter returned nil handler")
	}
}

func TestRateLimiterBenchmarkModeBypass(t *testing.T) {
	t.Setenv("BENCHMARK_MODE", "true")
	handler := RateLimiter()
	app := fiber.New()
	app.Use(handler)
	app.Get("/", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })
	for i := 0; i < 20; i++ {
		resp, err := app.Test(httptest.NewRequest(http.MethodGet, "/", http.NoBody))
		if err != nil {
			t.Fatal(err)
		}
		if resp.StatusCode != fiber.StatusOK {
			resp.Body.Close()
			t.Fatalf("status %d at iter %d", resp.StatusCode, i)
		}
		_, _ = io.Copy(io.Discard, resp.Body)
		resp.Body.Close()
	}
}

func TestRateLimiterDefaultsWhenZero(t *testing.T) {
	config.AppConfig = &config.Config{
		Server: config.ServerConfig{
			RateLimitMax:           0,
			RateLimitWindowSeconds: 0,
		},
	}

	handler := RateLimiter()
	if handler == nil {
		t.Fatal("RateLimiter returned nil handler with zero config")
	}
}
