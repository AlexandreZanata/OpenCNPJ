package admin

import (
	"io/fs"
	"net/http"

	"github.com/gofiber/fiber/v2"

	adminmw "busca-cnpj-2026/internal/adminauth/middleware"
	"busca-cnpj-2026/internal/adminauth/token"
	"busca-cnpj-2026/internal/middleware"
)

// RegisterRoutes mounts server-rendered admin panel routes.
func RegisterRoutes(app *fiber.App, h *Handler, adminHost string) error {
	staticFS, err := StaticFS()
	if err != nil {
		return err
	}
	adminApp := app.Group("/admin", middleware.AdminHostRequired(adminHost))
	adminApp.Get("/static/*", staticHandler(staticFS))

	adminApp.Get("/login", h.GetLogin)
	adminApp.Post("/login", h.ValidateCSRF, h.PostLogin)
	adminApp.Get("/mfa", h.GetMFA)
	adminApp.Post("/mfa", h.ValidateCSRF, h.PostMFA)
	adminApp.Post("/logout", h.ValidateCSRF, h.PostLogout)

	ra := h.requireAuth
	adminApp.Get("/", ra, h.GetDashboard)
	adminApp.Get("/clients", ra, h.GetClients)
	adminApp.Get("/clients/new", ra, h.GetClientNew)
	adminApp.Post("/clients", h.ValidateCSRF, ra, h.PostClient)
	adminApp.Get("/clients/:id", ra, h.GetClientDetail)
	adminApp.Post("/clients/:id/keys", h.ValidateCSRF, ra, h.PostCreateKey)
	adminApp.Post("/clients/:id/keys/:kid/revoke", h.ValidateCSRF, ra, h.PostRevokeKey)
	adminApp.Post("/clients/:id/suspend", h.ValidateCSRF, ra, h.PostSuspend)
	adminApp.Get("/usage", ra, h.GetUsage)
	return nil
}

// RegisterAPIRoutes mounts JSON admin API for SPA clients (Bearer JWT after MFA).
func RegisterAPIRoutes(
	app *fiber.App,
	h *Handler,
	signer *token.RS256Signer,
	adminHost string,
) {
	host := middleware.AdminHostRequired(adminHost)
	api := app.Group("/admin/api/v1", host, adminmw.Session(signer), adminmw.RequireMFA())
	api.Get("/dashboard", h.JSONDashboard)
	api.Get("/clients", h.JSONListClients)
	api.Post("/clients", h.JSONCreateClient)
	api.Get("/clients/:id", h.JSONGetClient)
	api.Post("/clients/:id/suspend", h.JSONSuspendClient)
	api.Post("/clients/:id/keys", h.JSONCreateKey)
	api.Post("/clients/:id/keys/:kid/revoke", h.JSONRevokeKey)
	api.Get("/usage", h.JSONUsage)
}

func (h *Handler) requireAuth(c *fiber.Ctx) error {
	sess, err := getSess(c, h.Session)
	if err != nil {
		return c.Redirect("/admin/login")
	}
	adminID, ok := sessGetUUID(sess, sessAdminID)
	tok, _ := sess.Get(sessAccessToken).(string)
	if !ok || tok == "" {
		return c.Redirect("/admin/login")
	}
	claims, err := h.Signer.ParseAccessToken(tok)
	if err != nil || !claims.MFAVerified {
		sessClearAuth(sess)
		_ = sess.Save()
		return c.Redirect("/admin/login")
	}
	c.Locals("adminID", adminID)
	return c.Next()
}

func staticHandler(fsys fs.FS) fiber.Handler {
	return func(c *fiber.Ctx) error {
		path := c.Params("*")
		if path == "" {
			return fiber.ErrNotFound
		}
		data, err := fs.ReadFile(fsys, path)
		if err != nil {
			return fiber.ErrNotFound
		}
		ctype := "application/octet-stream"
		if len(path) > 4 && path[len(path)-4:] == ".css" {
			ctype = "text/css; charset=utf-8"
		}
		c.Set("Content-Type", ctype)
		c.Set("Cache-Control", "public, max-age=86400")
		return c.Status(http.StatusOK).Send(data)
	}
}
