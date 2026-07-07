package middleware

import (
	"io"
	"net/http"
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

	req := httptest.NewRequest(fiber.MethodGet, "/ok", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatalf("app.Test: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	if cc := resp.Header.Get("Cache-Control"); cc != "public, max-age=300" {
		t.Fatalf("Cache-Control = %q", cc)
	}
	if resp.Header.Get("ETag") == "" {
		t.Fatal("expected ETag header")
	}
	_, _ = io.Copy(io.Discard, resp.Body)
}

func TestResponseCacheSetsPrivateOnCNPJRoute(t *testing.T) {
	app := fiber.New()
	app.Use(ResponseCache())
	app.Get("/api/v1/cnpj/:cnpj", func(c *fiber.Ctx) error {
		return c.SendString(`{"cnpj":"00000000000191"}`)
	})

	req := httptest.NewRequest(fiber.MethodGet, "/api/v1/cnpj/00000000000191", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if cc := resp.Header.Get("Cache-Control"); cc != "private, max-age=300" {
		t.Fatalf("Cache-Control = %q", cc)
	}
}

func TestResponseCacheSkipsPOST(t *testing.T) {
	app := fiber.New()
	app.Use(ResponseCache())
	app.Post("/ok", func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusCreated)
	})

	req := httptest.NewRequest(fiber.MethodPost, "/ok", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatalf("app.Test: %v", err)
	}
	defer resp.Body.Close()
	if resp.Header.Get("Cache-Control") != "" {
		t.Fatal("POST should not set Cache-Control")
	}
	_, _ = io.Copy(io.Discard, resp.Body)
}
