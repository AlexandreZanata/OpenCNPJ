package middleware

import (
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/domain"
)

func TestRequireMFARejectsUnverified(t *testing.T) {
	app := fiber.New()
	app.Get("/x", func(c *fiber.Ctx) error {
		c.Locals("adminSession", domain.SessionClaims{
			AdminID: uuid.New(), Role: "super_admin", MFAVerified: false,
		})
		return RequireMFA()(c)
	})
	req := httptest.NewRequest(http.MethodGet, "/x", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != fiber.StatusForbidden {
		t.Fatalf("status=%d", resp.StatusCode)
	}
}

func TestRequireMFAPassesVerified(t *testing.T) {
	app := fiber.New()
	app.Get("/x", func(c *fiber.Ctx) error {
		c.Locals("adminSession", domain.SessionClaims{
			AdminID: uuid.New(), Role: "super_admin", MFAVerified: true,
		})
		return RequireMFA()(c)
	}, func(c *fiber.Ctx) error {
		return c.SendStatus(fiber.StatusOK)
	})
	req := httptest.NewRequest(http.MethodGet, "/x", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status=%d", resp.StatusCode)
	}
}
