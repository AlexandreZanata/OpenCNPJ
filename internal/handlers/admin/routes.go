package admin

import (
	"io/fs"
	"net/http"

	"github.com/gofiber/fiber/v2"

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

	g := adminApp.Group("", h.requireAuth)
	g.Get("/", h.GetDashboard)
	g.Get("/clients", h.GetClients)
	g.Get("/clients/new", h.GetClientNew)
	g.Post("/clients", h.ValidateCSRF, h.PostClient)
	g.Get("/clients/:id", h.GetClientDetail)
	g.Post("/clients/:id/keys", h.ValidateCSRF, h.PostCreateKey)
	g.Post("/clients/:id/keys/:kid/revoke", h.ValidateCSRF, h.PostRevokeKey)
	g.Post("/clients/:id/suspend", h.ValidateCSRF, h.PostSuspend)
	g.Get("/usage", h.GetUsage)
	return nil
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
