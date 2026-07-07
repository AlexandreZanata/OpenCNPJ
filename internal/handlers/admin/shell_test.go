package admin

import (
	"bytes"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gofiber/fiber/v2"
)

func TestShellIncludesAPIDocsURL(t *testing.T) {
	app := fiber.New()
	h := &Handler{Deps: Deps{DocsPublicURL: "https://example.com/docs", Session: NewSession()}}
	var data LayoutData
	app.Get("/", func(c *fiber.Ctx) error {
		data = h.shell(c, "Dashboard", "dashboard", "dashboard-content", true)
		return nil
	})
	if _, err := app.Test(httptest.NewRequest(http.MethodGet, "/", http.NoBody)); err != nil {
		t.Fatal(err)
	}
	if data.APIDocsURL != "https://example.com/docs" {
		t.Fatalf("url = %q", data.APIDocsURL)
	}
}

func TestLayoutRendersAPIDocsLink(t *testing.T) {
	r, err := NewRenderer()
	if err != nil {
		t.Fatal(err)
	}
	var buf bytes.Buffer
	err = r.Render(&buf, "dashboard.html", dashboardPage{
		LayoutData: LayoutData{
			Title: "Dashboard", Nav: "dashboard", ContentTpl: "dashboard-content",
			APIDocsURL: "https://example.com/api-docs",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	out := buf.String()
	if !strings.Contains(out, "API docs") || !strings.Contains(out, "https://example.com/api-docs") {
		t.Fatalf("layout missing API docs link: %s", out)
	}
}
