package admin

import (
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/session"
	"github.com/google/uuid"
)

const (
	sessChallengeID = "challenge_id"
	sessAdminID     = "admin_id"
	sessAccessToken = "access_token"
	sessNewKey      = "new_key_flash"
)

// NewSession creates an HttpOnly cookie session store.
func NewSession() *session.Store {
	return session.New(session.Config{
		KeyLookup:      "cookie:opencnpj_admin_session",
		CookieHTTPOnly: true,
		CookieSameSite: "Strict",
		Expiration:     12 * time.Hour,
	})
}

func getSess(c *fiber.Ctx, store *session.Store) (*session.Session, error) {
	return store.Get(c)
}

func sessGetUUID(s *session.Session, key string) (uuid.UUID, bool) {
	v := s.Get(key)
	if v == nil {
		return uuid.Nil, false
	}
	str, ok := v.(string)
	if !ok || str == "" {
		return uuid.Nil, false
	}
	id, err := uuid.Parse(str)
	if err != nil {
		return uuid.Nil, false
	}
	return id, true
}

func sessSetUUID(s *session.Session, key string, id uuid.UUID) {
	s.Set(key, id.String())
}

func sessClearAuth(s *session.Session) {
	s.Delete(sessChallengeID)
	s.Delete(sessAdminID)
	s.Delete(sessAccessToken)
}
