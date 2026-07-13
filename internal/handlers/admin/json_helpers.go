package admin

import (
	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	adminmw "busca-cnpj-2026/internal/adminauth/middleware"
)

func adminIDFromJWT(c *fiber.Ctx) uuid.UUID {
	claims, ok := adminmw.SessionFromCtx(c)
	if !ok {
		return uuid.Nil
	}
	return claims.AdminID
}

func jsonErr(c *fiber.Ctx, status int, code, message string) error {
	return c.Status(status).JSON(fiber.Map{"error": code, "message": message, "code": status})
}
