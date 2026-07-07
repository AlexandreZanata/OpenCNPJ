package middleware_test

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/middleware"
)

func TestAdminHostRequiredBlocksWrongHost(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.AdminHostRequired("admin.example.com"))
	app.Get("/admin/", func(c *fiber.Ctx) error { return c.SendString("ok") })

	req := httptest.NewRequest(http.MethodGet, "http://api.example.com/admin/", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	if resp.StatusCode != fiber.StatusNotFound {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestAdminHostRequiredAllowsMatch(t *testing.T) {
	app := fiber.New()
	app.Use(middleware.AdminHostRequired("admin.example.com"))
	app.Get("/admin/", func(c *fiber.Ctx) error { return c.SendString("ok") })

	req := httptest.NewRequest(http.MethodGet, "http://admin.example.com/admin/", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}
