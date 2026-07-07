package admin_test

import (
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/session"

	"busca-cnpj-2026/internal/handlers/admin"
)

func TestCSRFBlocksPostWithoutToken(t *testing.T) {
	store := session.New(session.Config{KeyLookup: "cookie:s", Expiration: time.Hour})
	h := admin.NewHandler(admin.Deps{Session: store, Renderer: admin.MustRenderer()})
	app := fiber.New()
	app.Post("/admin/logout", h.ValidateCSRF, h.PostLogout)

	req := httptest.NewRequest(http.MethodPost, "/admin/logout", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	if resp.StatusCode != fiber.StatusForbidden {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestCSRFAllowsPostWithToken(t *testing.T) {
	store := session.New(session.Config{KeyLookup: "cookie:s", Expiration: time.Hour})
	h := admin.NewHandler(admin.Deps{
		Session: store, RefreshCookie: "r", Renderer: admin.MustRenderer(),
	})
	app := fiber.New()
	app.Get("/admin/login", h.GetLogin)
	app.Post("/admin/logout", h.ValidateCSRF, h.PostLogout)

	loginPage := httptest.NewRequest(http.MethodGet, "/admin/login", http.NoBody)
	loginResp, err := app.Test(loginPage)
	if err != nil {
		t.Fatal(err)
	}
	body := readHTMLBody(t, loginResp)
	token := extractCSRF(body)
	if token == "" {
		t.Fatal("missing csrf in login form")
	}

	vals := url.Values{"_csrf": {token}}
	req := httptest.NewRequest(http.MethodPost, "/admin/logout", strings.NewReader(vals.Encode()))
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	for _, c := range loginResp.Cookies() {
		req.AddCookie(c)
	}
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
	if resp.StatusCode != fiber.StatusFound {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func extractCSRF(html string) string {
	const marker = `name="_csrf" value="`
	i := strings.Index(html, marker)
	if i < 0 {
		return ""
	}
	rest := html[i+len(marker):]
	end := strings.Index(rest, `"`)
	if end < 0 {
		return ""
	}
	return rest[:end]
}

func readHTMLBody(t *testing.T, resp *http.Response) string {
	t.Helper()
	b, err := io.ReadAll(resp.Body)
	if err != nil {
		t.Fatal(err)
	}
	_ = resp.Body.Close()
	return string(b)
}
