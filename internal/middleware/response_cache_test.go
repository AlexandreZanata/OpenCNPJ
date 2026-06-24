package middleware

import (
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"
)

func TestResponseCacheSetsHeadersOnGET(t *testing.T) {
	app := fiber.New()
	app.Use(ResponseCache())
	app.Get("/ok", func(c *fiber.Ctx) error {
		return c.SendString(`{"status":"ok"}`)
	})

	req := httptest.NewRequest(fiber.MethodGet, "/ok", nil)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatalf("app.Test: %v", err)
	}
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	if cc := resp.Header.Get("Cache-Control"); cc != "public, max-age=300" {
		t.Fatalf("Cache-Control = %q", cc)
	}
	if resp.Header.Get("ETag") == "" {
		t.Fatal("expected ETag header")
	}
}

func TestResponseCacheSkipsPOST(t *testing.T) {
	app := fiber.New()
	app.Use(ResponseCache())
	app.Post("/ok", func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusCreated)
	})

	req := httptest.NewRequest(fiber.MethodPost, "/ok", nil)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatalf("app.Test: %v", err)
	}
	if resp.Header.Get("Cache-Control") != "" {
		t.Fatal("POST should not set Cache-Control")
	}
}
