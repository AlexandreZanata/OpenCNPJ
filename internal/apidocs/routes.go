package apidocs

import (
	"io/fs"

	"github.com/gofiber/fiber/v2"
)

// RegisterRoutes mounts Redoc UI at /docs when API docs are enabled.
func RegisterRoutes(app *fiber.App) error {
	sub, err := fs.Sub(static, "static")
	if err != nil {
		return err
	}
	docs := app.Group("/docs")
	docs.Get("/", func(c *fiber.Ctx) error {
		data, err := fs.ReadFile(sub, "index.html")
		if err != nil {
			return fiber.NewError(fiber.StatusInternalServerError, "docs index missing")
		}
		c.Set(fiber.HeaderContentType, fiber.MIMETextHTMLCharsetUTF8)
		return c.Send(data)
	})
	docs.Get("/openapi.yaml", func(c *fiber.Ctx) error {
		data, err := fs.ReadFile(sub, "openapi.yaml")
		if err != nil {
			return fiber.NewError(fiber.StatusInternalServerError, "openapi spec missing")
		}
		c.Set(fiber.HeaderContentType, "application/yaml; charset=utf-8")
		return c.Send(data)
	})
	return nil
}
