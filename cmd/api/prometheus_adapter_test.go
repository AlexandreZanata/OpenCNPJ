package main

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

func TestMetricsEndpointServesWithHTTPRequest(t *testing.T) {
	app := fiber.New()
	app.Get("/metrics", func(c *fiber.Ctx) error {
		adapter := newFiberPrometheusAdapter(c)
		req, err := http.NewRequestWithContext(c.Context(), http.MethodGet, "/metrics", http.NoBody)
		if err != nil {
			return err
		}
		promhttp.Handler().ServeHTTP(adapter, req)
		return nil
	})

	resp, err := app.Test(httptest.NewRequest(http.MethodGet, "/metrics", http.NoBody))
	if err != nil {
		t.Fatalf("app.Test: %v", err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	body, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatalf("read body: %v", err)
	}
	if len(body) == 0 {
		t.Fatal("empty metrics body")
	}
}
