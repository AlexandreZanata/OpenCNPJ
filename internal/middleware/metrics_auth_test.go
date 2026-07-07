package middleware_test

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/middleware"
)

func TestMetricsAuthBearerToken(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.MetricsAuth("secret-token", false))
	app.Get("/metrics", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	bad := httptest.NewRequest(http.MethodGet, "/metrics", http.NoBody)
	resp, err := app.Test(bad)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != fiber.StatusUnauthorized {
		t.Fatalf("status = %d", resp.StatusCode)
	}

	good := httptest.NewRequest(http.MethodGet, "/metrics", http.NoBody)
	good.Header.Set("Authorization", "Bearer secret-token")
	resp, err = app.Test(good)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestMetricsAuthInternalOnly(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.MetricsAuth("", true))
	app.Get("/metrics", func(c *fiber.Ctx) error { return c.SendStatus(fiber.StatusOK) })

	req := httptest.NewRequest(http.MethodGet, "/metrics", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != fiber.StatusForbidden {
		t.Fatalf("public IP should be forbidden, status = %d", resp.StatusCode)
	}
}
