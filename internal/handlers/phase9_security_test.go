package handlers_test

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/handlers"
	"busca-cnpj-2026/internal/saas"
)

// Phase 9 pen-test gate: /api/v1/cnpj/* must not bypass API key auth.
func TestPhase9CNPJRouteRequiresAPIKey(t *testing.T) {
	config.AppConfig = &config.Config{
		SaaS: config.SaasConfig{Enabled: true, PublicAPIOnly: true},
	}
	app := fiber.New()
	deps := &saas.Deps{Auth: denyAuth{}}
	handlers.RegisterV1Routes(app, nil, nil, nil, nil, &handlers.CNPJHandler{}, deps)

	req := httptest.NewRequest(http.MethodGet, "/api/v1/cnpj/00000000000191", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	_, _ = io.Copy(io.Discard, resp.Body)
	if resp.StatusCode != fiber.StatusUnauthorized {
		t.Fatalf("expected 401 without API key, got %d", resp.StatusCode)
	}
}

type denyAuth struct{}

func (denyAuth) Authenticate(context.Context, string) (saas.AuthenticatedClient, error) {
	return saas.AuthenticatedClient{}, saas.ErrMissingKey
}
