package handlers

import (
	"github.com/gofiber/fiber/v2"

	adminmw "busca-cnpj-2026/internal/adminauth/middleware"
	"busca-cnpj-2026/internal/adminauth/token"
)

// RegisterRoutes mounts admin auth HTTP routes.
func RegisterRoutes(app *fiber.App, auth *AuthHandler, signer *token.RS256Signer) {
	g := app.Group("/admin/api/v1/auth")
	g.Post("/login", auth.PostLogin)
	g.Post("/mfa/verify", auth.PostMFAVerify)
	g.Post("/refresh", auth.PostRefresh)

	protected := app.Group("/admin/api/v1", adminmw.Session(signer), adminmw.RequireMFA())
	protected.Get("/me", auth.GetMe)
}
