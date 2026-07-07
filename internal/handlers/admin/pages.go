package admin

import (
	"errors"
	"strings"
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/audit"
	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/usecase"
)

type loginPage struct {
	Error     string
	CSRFToken string
}

type mfaPage struct {
	Error     string
	CSRFToken string
}

type dashboardPage struct {
	LayoutData
	TotalClients  int64
	RequestsToday int64
}

// GetLogin shows the login form.
func (h *Handler) GetLogin(c *fiber.Ctx) error {
	token, _ := h.csrfToken(c)
	return h.html(c, "login.html", loginPage{CSRFToken: token})
}

// PostLogin handles credential submission.
func (h *Handler) PostLogin(c *fiber.Ctx) error {
	email := strings.TrimSpace(c.FormValue("email"))
	pass := c.FormValue("password")
	out, err := h.Login(c.Context(), usecase.LoginInput{Email: email, Password: pass})
	if err != nil {
		token, _ := h.csrfToken(c)
		msg := "Invalid credentials"
		if errors.Is(err, autherr.ErrAccountLocked) {
			msg = "Account temporarily locked"
		}
		_ = h.logAudit(c, uuid.Nil, audit.ActionLoginFailure, "admin_user", email)
		return h.html(c, "login.html", loginPage{Error: msg, CSRFToken: token})
	}
	sess, err := getSess(c, h.Session)
	if err != nil {
		return err
	}
	sessSetUUID(sess, sessChallengeID, out.ChallengeID)
	if err := sess.Save(); err != nil {
		return err
	}
	return c.Redirect("/admin/mfa")
}

// GetMFA shows the TOTP form.
func (h *Handler) GetMFA(c *fiber.Ctx) error {
	sess, err := getSess(c, h.Session)
	if err != nil {
		return err
	}
	if _, ok := sessGetUUID(sess, sessChallengeID); !ok {
		return c.Redirect("/admin/login")
	}
	token, _ := h.csrfToken(c)
	return h.html(c, "mfa.html", mfaPage{CSRFToken: token})
}

// PostMFA verifies TOTP and establishes session.
func (h *Handler) PostMFA(c *fiber.Ctx) error {
	sess, err := getSess(c, h.Session)
	if err != nil {
		return err
	}
	chID, ok := sessGetUUID(sess, sessChallengeID)
	if !ok {
		return c.Redirect("/admin/login")
	}
	tokens, err := h.Verify(c.Context(), usecase.VerifyMFAInput{
		ChallengeID: chID,
		Code:        strings.TrimSpace(c.FormValue("code")),
	})
	if err != nil {
		token, _ := h.csrfToken(c)
		msg := "Invalid code"
		return h.html(c, "mfa.html", mfaPage{Error: msg, CSRFToken: token})
	}
	claims, err := h.Signer.ParseAccessToken(tokens.AccessToken)
	if err != nil {
		token, _ := h.csrfToken(c)
		return h.html(c, "mfa.html", mfaPage{Error: "Session error", CSRFToken: token})
	}
	sess.Delete(sessChallengeID)
	sessSetUUID(sess, sessAdminID, claims.AdminID)
	sess.Set(sessAccessToken, tokens.AccessToken)
	if err := sess.Save(); err != nil {
		return err
	}
	_ = h.logAudit(c, claims.AdminID, audit.ActionMFAVerified, "admin_user", claims.AdminID.String())
	_ = h.logAudit(c, claims.AdminID, audit.ActionLoginSuccess, "admin_user", claims.AdminID.String())
	setRefreshCookie(c, h.RefreshCookie, tokens.RefreshToken, tokens.RefreshExpires)
	return c.Redirect("/admin/")
}

// PostLogout clears admin session.
func (h *Handler) PostLogout(c *fiber.Ctx) error {
	sess, err := getSess(c, h.Session)
	if err == nil {
		sessClearAuth(sess)
		_ = sess.Save()
	}
	c.ClearCookie(h.RefreshCookie)
	return c.Redirect("/admin/login")
}

// GetDashboard shows summary stats.
func (h *Handler) GetDashboard(c *fiber.Ctx) error {
	ctx := c.Context()
	clients, err := h.Queries.CountAPIClients(ctx)
	if err != nil {
		return err
	}
	today, err := h.Queries.SumUsageRequestsToday(ctx)
	if err != nil {
		return err
	}
	return h.html(c, "dashboard.html", dashboardPage{
		LayoutData:    h.shell(c, "Dashboard", "dashboard", "dashboard-content", true),
		TotalClients:  clients,
		RequestsToday: today,
	})
}

func (h *Handler) html(c *fiber.Ctx, name string, data any) error {
	c.Set("Content-Type", "text/html; charset=utf-8")
	return h.Renderer.Render(c, name, data)
}

func setRefreshCookie(c *fiber.Ctx, name, value string, expires time.Time) {
	c.Cookie(&fiber.Cookie{
		Name:     name,
		Value:    value,
		Path:     "/",
		HTTPOnly: true,
		Secure:   c.Protocol() == "https",
		SameSite: "Strict",
		Expires:  expires,
	})
}
