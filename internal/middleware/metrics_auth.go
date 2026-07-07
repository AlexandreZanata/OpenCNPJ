package middleware

import (
	"crypto/subtle"
	"net"
	"strings"

	"github.com/gofiber/fiber/v2"
)

// MetricsAuth protects /metrics with bearer token and/or private-network source IP.
func MetricsAuth(bearerToken string, internalOnly bool) fiber.Handler {
	token := strings.TrimSpace(bearerToken)
	return func(c *fiber.Ctx) error {
		if token != "" {
			auth := strings.TrimSpace(c.Get("Authorization"))
			want := "Bearer " + token
			if subtle.ConstantTimeCompare([]byte(auth), []byte(want)) != 1 {
				return c.Status(fiber.StatusUnauthorized).JSON(fiber.Map{
					"error": "unauthorized",
				})
			}
			return c.Next()
		}
		if internalOnly && !isPrivateIP(c.IP()) {
			return c.Status(fiber.StatusForbidden).JSON(fiber.Map{
				"error": "forbidden",
			})
		}
		return c.Next()
	}
}

func isPrivateIP(raw string) bool {
	ip := net.ParseIP(strings.TrimSpace(raw))
	if ip == nil {
		return false
	}
	if ip.IsLoopback() {
		return true
	}
	if ip4 := ip.To4(); ip4 != nil {
		switch {
		case ip4[0] == 10:
			return true
		case ip4[0] == 172 && ip4[1] >= 16 && ip4[1] <= 31:
			return true
		case ip4[0] == 192 && ip4[1] == 168:
			return true
		case ip4[0] == 127:
			return true
		default:
			return false
		}
	}
	return ip.IsLinkLocalUnicast() || ip.IsLinkLocalMulticast()
}
