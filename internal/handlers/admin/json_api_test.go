package admin_test

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/handlers/admin"
)

func TestRegisterAPIRoutesMountsManagementPaths(t *testing.T) {
	app := fiber.New()
	h := &admin.Handler{}
	admin.RegisterAPIRoutes(app, h, nil, "")

	for _, p := range []string{
		"/admin/api/v1/dashboard",
		"/admin/api/v1/clients",
		"/admin/api/v1/usage",
	} {
		r := httptest.NewRequest(fiber.MethodGet, p, http.NoBody)
		resp, err := app.Test(r, -1)
		if err != nil {
			t.Fatalf("%s: %v", p, err)
		}
		if resp.StatusCode == fiber.StatusNotFound {
			t.Fatalf("route not registered: %s", p)
		}
	}
}
