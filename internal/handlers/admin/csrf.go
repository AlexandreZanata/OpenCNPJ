package admin

import (
	"crypto/rand"
	"crypto/subtle"
	"encoding/base64"
	"fmt"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/session"
)

const sessCSRF = "csrf_token"

// ensureCSRFToken returns the session CSRF token, generating one when missing.
func ensureCSRFToken(sess *session.Session) (string, error) {
	if v := sess.Get(sessCSRF); v != nil {
		if s, ok := v.(string); ok && s != "" {
			return s, nil
		}
	}
	buf := make([]byte, 32)
	if _, err := rand.Read(buf); err != nil {
		return "", fmt.Errorf("csrf rand: %w", err)
	}
	token := base64.RawURLEncoding.EncodeToString(buf)
	sess.Set(sessCSRF, token)
	return token, nil
}

// ValidateCSRF checks the _csrf form field on POST requests.
func (h *Handler) ValidateCSRF(c *fiber.Ctx) error {
	if c.Method() != fiber.MethodPost {
		return c.Next()
	}
	sess, err := getSess(c, h.Session)
	if err != nil {
		return fiber.ErrForbidden
	}
	want, err := ensureCSRFToken(sess)
	if err != nil {
		return err
	}
	got := c.FormValue("_csrf")
	if subtle.ConstantTimeCompare([]byte(want), []byte(got)) != 1 {
		return c.Status(fiber.StatusForbidden).SendString("Invalid CSRF token")
	}
	return c.Next()
}

func (h *Handler) csrfToken(c *fiber.Ctx) (string, error) {
	sess, err := getSess(c, h.Session)
	if err != nil {
		return "", err
	}
	token, err := ensureCSRFToken(sess)
	if err != nil {
		return "", err
	}
	if err := sess.Save(); err != nil {
		return "", err
	}
	return token, nil
}
