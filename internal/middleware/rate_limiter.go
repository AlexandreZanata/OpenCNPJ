package middleware

import (
	"time"

	"busca-cnpj-2026/internal/config"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/limiter"
)

func RateLimiter() fiber.Handler {
	max := config.AppConfig.Server.RateLimitMax
	if max <= 0 {
		max = 6000
	}
	window := config.AppConfig.Server.RateLimitWindowSeconds
	if window <= 0 {
		window = 60
	}

	return limiter.New(limiter.Config{
		Max:        max,
		Expiration: time.Duration(window) * time.Second,
		KeyGenerator: func(c *fiber.Ctx) string {
			return c.IP()
		},
		LimitReached: func(c *fiber.Ctx) error {
			return c.Status(fiber.StatusTooManyRequests).JSON(fiber.Map{
				"error":   "rate_limit_exceeded",
				"message": "Too many requests",
			})
		},
	})
}
