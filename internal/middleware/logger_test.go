package middleware_test

import (
	"bytes"
	"io"
	"log"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/middleware"
	"busca-cnpj-2026/internal/saas"
)

func TestLoggerMasksAPIKeyHeader(t *testing.T) {
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(io.Discard)

	app := fiber.New()
	app.Use(middleware.RequestID())
	app.Use(middleware.Logger())
	app.Get("/", func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusOK)
	})

	raw := saas.KeyPrefix + strings.Repeat("a", saas.KeyHexLength)
	req := httptest.NewRequest(http.MethodGet, "/", http.NoBody)
	req.Header.Set(saas.HeaderAPIKey, raw)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)

	out := buf.String()
	if strings.Contains(out, raw) {
		t.Fatalf("log leaked full api key: %s", out)
	}
	if !strings.Contains(out, saas.MaskAPIKey(raw)) {
		t.Fatalf("expected masked key in log: %s", out)
	}
}
