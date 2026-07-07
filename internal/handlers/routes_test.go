package handlers

import (
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/config"
)

func TestRegisterV1RoutesPublicAPIOnly(t *testing.T) {
	config.AppConfig = &config.Config{
		SaaS: config.SaasConfig{Enabled: true, PublicAPIOnly: true},
	}
	app := fiber.New()
	RegisterV1Routes(app, &SearchHandler{}, &ExportHandler{}, &LookupHandler{}, &StatsHandler{}, nil)

	routes := collectRoutePaths(app)
	if !containsPath(routes, "/api/v1/cnpj/:cnpj") {
		t.Fatalf("missing cnpj route, got %v", routes)
	}
	if containsPath(routes, "/api/v1/empresas/search") {
		t.Fatalf("empresas search should be disabled in public_api_only mode")
	}
}

func TestRegisterV1RoutesFullAPI(t *testing.T) {
	config.AppConfig = &config.Config{
		SaaS: config.SaasConfig{Enabled: false},
	}
	app := fiber.New()
	RegisterV1Routes(app, &SearchHandler{}, &ExportHandler{}, &LookupHandler{}, &StatsHandler{}, nil)

	routes := collectRoutePaths(app)
	if !containsPath(routes, "/api/v1/empresas/search") {
		t.Fatalf("missing empresas search in full API mode")
	}
}

func collectRoutePaths(app *fiber.App) []string {
	var paths []string
	for _, r := range app.GetRoutes() {
		paths = append(paths, r.Path)
	}
	return paths
}

func containsPath(paths []string, want string) bool {
	for _, p := range paths {
		if p == want {
			return true
		}
	}
	return false
}
