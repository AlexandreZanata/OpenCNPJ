package middleware

import (
	"crypto/sha256"
	"encoding/hex"
	"strings"

	"github.com/gofiber/fiber/v2"
)

// ResponseCache sets Cache-Control and ETag on successful GET responses.
func ResponseCache() fiber.Handler {
	return func(c *fiber.Ctx) error {
		if c.Method() != fiber.MethodGet {
			return c.Next()
		}

		if err := c.Next(); err != nil {
			return err
		}
		if c.Response().StatusCode() != fiber.StatusOK {
			return nil
		}

		body := c.Response().Body()
		if len(body) == 0 {
			return nil
		}

		sum := sha256.Sum256(body)
		cc := "public, max-age=300"
		if strings.HasPrefix(c.Path(), "/api/v1/cnpj/") {
			cc = "private, max-age=300"
		}
		c.Set("Cache-Control", cc)
		c.Set("ETag", `"`+hex.EncodeToString(sum[:8])+`"`)
		return nil
	}
}
