package handlers

import (
	"context"
	"errors"
	"strings"
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/autherr"
	adminmw "busca-cnpj-2026/internal/adminauth/middleware"
	"busca-cnpj-2026/internal/adminauth/usecase"
)

// AuthHandler exposes admin login and MFA endpoints.
type AuthHandler struct {
	login   loginFn
	verify  verifyFn
	refresh refreshFn
	cfg     string
}

type loginFn func(ctx context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error)
type verifyFn func(ctx context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error)
type refreshFn func(ctx context.Context, rawRefresh string) (usecase.AuthTokens, error)

// NewAuthHandler wires HTTP handlers to use cases.
func NewAuthHandler(
	login loginFn,
	verify verifyFn,
	refresh refreshFn,
	refreshCookieName string,
) *AuthHandler {
	return &AuthHandler{login: login, verify: verify, refresh: refresh, cfg: refreshCookieName}
}

type loginBody struct {
	Email    string `json:"email"`
	Password string `json:"password"`
}

type verifyBody struct {
	ChallengeID string `json:"challengeId"`
	Code        string `json:"code"`
}

// Login runs the credential step (for HTML panel and JSON API).
func (h *AuthHandler) Login(ctx context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error) {
	return h.login(ctx, in)
}

// Verify runs MFA verification (for HTML panel and JSON API).
func (h *AuthHandler) Verify(ctx context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error) {
	return h.verify(ctx, in)
}

// PostLogin handles POST /admin/api/v1/auth/login.
func (h *AuthHandler) PostLogin(c *fiber.Ctx) error {
	var body loginBody
	if err := c.BodyParser(&body); err != nil {
		return badRequest(c, "invalid_json")
	}
	out, err := h.Login(c.Context(), usecase.LoginInput{
		Email:    strings.TrimSpace(body.Email),
		Password: body.Password,
	})
	if err != nil {
		return mapAuthErr(c, err)
	}
	return c.JSON(fiber.Map{
		"status":           "mfa_required",
		"challengeId":      out.ChallengeID.String(),
		"expiresInSeconds": out.ExpiresInSeconds,
	})
}

// PostMFAVerify handles POST /admin/api/v1/auth/mfa/verify.
func (h *AuthHandler) PostMFAVerify(c *fiber.Ctx) error {
	var body verifyBody
	if err := c.BodyParser(&body); err != nil {
		return badRequest(c, "invalid_json")
	}
	chID, err := uuid.Parse(strings.TrimSpace(body.ChallengeID))
	if err != nil {
		return badRequest(c, "invalid_challenge_id")
	}
	tokens, err := h.Verify(c.Context(), usecase.VerifyMFAInput{ChallengeID: chID, Code: body.Code})
	if err != nil {
		return mapAuthErr(c, err)
	}
	setRefreshCookie(c, h.cfg, tokens.RefreshToken, tokens.RefreshExpires)
	return c.JSON(fiber.Map{
		"accessToken":      tokens.AccessToken,
		"expiresInSeconds": tokens.ExpiresInSeconds,
	})
}

// PostRefresh handles POST /admin/api/v1/auth/refresh.
func (h *AuthHandler) PostRefresh(c *fiber.Ctx) error {
	raw := c.Cookies(h.cfg)
	if raw == "" {
		return mapAuthErr(c, autherr.ErrInvalidToken)
	}
	tokens, err := h.refresh(c.Context(), raw)
	if err != nil {
		return mapAuthErr(c, err)
	}
	setRefreshCookie(c, h.cfg, tokens.RefreshToken, tokens.RefreshExpires)
	return c.JSON(fiber.Map{
		"accessToken":      tokens.AccessToken,
		"expiresInSeconds": tokens.ExpiresInSeconds,
	})
}

// GetMe is a protected sample route requiring MFA-verified JWT.
func (h *AuthHandler) GetMe(c *fiber.Ctx) error {
	claims, ok := adminmw.SessionFromCtx(c)
	if !ok {
		return mapAuthErr(c, autherr.ErrInvalidToken)
	}
	return c.JSON(fiber.Map{
		"adminId": claims.AdminID.String(),
		"role":    claims.Role,
	})
}

func setRefreshCookie(c *fiber.Ctx, name, value string, expires time.Time) {
	c.Cookie(&fiber.Cookie{
		Name:     name,
		Value:    value,
		Path:     "/admin/api/v1/auth",
		HTTPOnly: true,
		Secure:   c.Protocol() == "https",
		SameSite: "Strict",
		Expires:  expires,
	})
}

func mapAuthErr(c *fiber.Ctx, err error) error {
	switch {
	case errors.Is(err, autherr.ErrInvalidCredentials):
		return c.Status(fiber.StatusUnauthorized).JSON(errBody("invalid_credentials", err))
	case errors.Is(err, autherr.ErrAccountLocked):
		return c.Status(fiber.StatusTooManyRequests).JSON(errBody("account_locked", err))
	case errors.Is(err, autherr.ErrInvalidMFA), errors.Is(err, autherr.ErrInvalidChallenge):
		return c.Status(fiber.StatusUnauthorized).JSON(errBody("invalid_mfa", err))
	case errors.Is(err, autherr.ErrInvalidToken):
		return c.Status(fiber.StatusUnauthorized).JSON(errBody("invalid_token", err))
	case errors.Is(err, autherr.ErrMFANotVerified):
		return c.Status(fiber.StatusForbidden).JSON(errBody("mfa_not_verified", err))
	default:
		return c.Status(fiber.StatusInternalServerError).JSON(errBody("internal_error", err))
	}
}

func badRequest(c *fiber.Ctx, code string) error {
	return c.Status(fiber.StatusBadRequest).JSON(fiber.Map{
		"error": code, "message": code, "code": fiber.StatusBadRequest,
	})
}

func errBody(code string, err error) fiber.Map {
	return fiber.Map{"error": code, "message": err.Error()}
}
