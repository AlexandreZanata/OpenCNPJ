package middleware

import (
	"log"
	"strings"
	"time"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/saas"
)

func Logger() fiber.Handler {
	return func(c *fiber.Ctx) error {
		start := time.Now()
		err := c.Next()
		rid, _ := c.Locals("request_id").(string)
		if rid == "" {
			rid = "-"
		}
		path := sanitizeLogPath(c.Path())
		if key := c.Get(saas.HeaderAPIKey); key != "" {
			log.Printf("[%s] %d %s %s api_key=%s %v",
				rid, c.Response().StatusCode(), c.Method(), path, saas.MaskAPIKey(key), time.Since(start))
		} else {
			log.Printf("[%s] %d %s %s %v",
				rid, c.Response().StatusCode(), c.Method(), path, time.Since(start))
		}
		return err
	}
}

func RequestID() fiber.Handler {
	return func(c *fiber.Ctx) error {
		requestID := c.Get("X-Request-ID")
		if requestID == "" {
			requestID = generateRequestID()
		}
		c.Set("X-Request-ID", requestID)
		c.Locals("request_id", requestID)
		return c.Next()
	}
}

func generateRequestID() string {
	return time.Now().Format("20060102150405") + "-" + randomString(8)
}

func randomString(length int) string {
	const charset = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
	b := make([]byte, length)
	for i := range b {
		b[i] = charset[time.Now().UnixNano()%int64(len(charset))]
	}
	return string(b)
}

func sanitizeLogPath(path string) string {
	if !strings.Contains(path, "ocnpj_live_") {
		return path
	}
	parts := strings.Split(path, "/")
	for i, p := range parts {
		if strings.HasPrefix(p, saas.KeyPrefix) {
			parts[i] = saas.MaskAPIKey(p)
		}
	}
	return strings.Join(parts, "/")
}
