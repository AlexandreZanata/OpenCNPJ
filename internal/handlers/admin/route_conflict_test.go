package admin_test

import (
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/handlers/admin"
)

func TestHTMLRequireAuthDoesNotCaptureAPIRoutes(t *testing.T) {
	app := fiber.New()
	h := &admin.Handler{}
	if err := admin.RegisterRoutes(app, h, ""); err != nil {
		t.Fatal(err)
	}
	admin.RegisterAPIRoutes(app, h, nil, "")

	req := httptest.NewRequest(http.MethodGet, "/admin/api/v1/dashboard", http.NoBody)
	resp, err := app.Test(req, -1)
	if err != nil {
		t.Fatal(err)
	}
	body, _ := io.ReadAll(resp.Body)
	if resp.StatusCode == fiber.StatusFound && resp.Header.Get("Location") == "/admin/login" {
		t.Fatalf("HTML requireAuth captured API route: status=%d body=%q", resp.StatusCode, body)
	}
}
