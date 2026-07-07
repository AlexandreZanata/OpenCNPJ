package middleware

import (
	"strings"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/domain"
	"busca-cnpj-2026/internal/adminauth/token"
)

const sessionKey = "adminSession"

// Session parses Bearer JWT and stores session claims in context.
func Session(signer *token.RS256Signer) fiber.Handler {
	return func(c *fiber.Ctx) error {
		raw := bearerToken(c.Get("Authorization"))
		if raw == "" {
			return authError(c, fiber.StatusUnauthorized, "missing_token", autherr.ErrInvalidToken)
		}
		claims, err := signer.ParseAccessToken(raw)
		if err != nil {
			return authError(c, fiber.StatusUnauthorized, "invalid_token", autherr.ErrInvalidToken)
		}
		c.Locals(sessionKey, claims)
		return c.Next()
	}
}

// RequireMFA rejects tokens without mfaVerified=true.
func RequireMFA() fiber.Handler {
	return func(c *fiber.Ctx) error {
		claims, ok := SessionFromCtx(c)
		if !ok {
			return authError(c, fiber.StatusUnauthorized, "missing_token", autherr.ErrInvalidToken)
		}
		if !claims.MFAVerified {
			return authError(c, fiber.StatusForbidden, "mfa_not_verified", autherr.ErrMFANotVerified)
		}
		return c.Next()
	}
}

// SessionFromCtx returns parsed JWT claims from Fiber locals.
func SessionFromCtx(c *fiber.Ctx) (domain.SessionClaims, bool) {
	v := c.Locals(sessionKey)
	if v == nil {
		return domain.SessionClaims{}, false
	}
	claims, ok := v.(domain.SessionClaims)
	return claims, ok
}

func bearerToken(header string) string {
	const prefix = "Bearer "
	if !strings.HasPrefix(header, prefix) {
		return ""
	}
	return strings.TrimSpace(strings.TrimPrefix(header, prefix))
}

func authError(c *fiber.Ctx, code int, errCode string, err error) error {
	return c.Status(code).JSON(fiber.Map{
		"error":   errCode,
		"message": err.Error(),
		"code":    code,
	})
}
