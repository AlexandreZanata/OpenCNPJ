package middleware

import (
	"strings"

	"github.com/gofiber/fiber/v2"
)

// AdminHostRequired restricts routes to a configured admin hostname (e.g. admin.example.com).
func AdminHostRequired(host string) fiber.Handler {
	want := strings.ToLower(strings.TrimSpace(host))
	if want == "" {
		return func(c *fiber.Ctx) error { return c.Next() }
	}
	return func(c *fiber.Ctx) error {
		got := strings.ToLower(c.Hostname())
		if got != want {
			return c.Status(fiber.StatusNotFound).SendString("Not Found")
		}
		return c.Next()
	}
}
