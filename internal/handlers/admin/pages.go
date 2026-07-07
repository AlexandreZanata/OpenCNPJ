package admin

import (
	"strings"
	"time"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/usecase"
)

type loginPage struct {
	Error string
}

type mfaPage struct {
	Error string
}

type dashboardPage struct {
	LayoutData
	TotalClients  int64
	RequestsToday int64
}

// GetLogin shows the login form.
func (h *Handler) GetLogin(c *fiber.Ctx) error {
	return h.html(c, "login.html", loginPage{})
}

// PostLogin handles credential submission.
func (h *Handler) PostLogin(c *fiber.Ctx) error {
	email := strings.TrimSpace(c.FormValue("email"))
	pass := c.FormValue("password")
	out, err := h.Login(c.Context(), usecase.LoginInput{Email: email, Password: pass})
	if err != nil {
		msg := "Invalid credentials"
		if err == autherr.ErrAccountLocked {
			msg = "Account temporarily locked"
		}
		return h.html(c, "login.html", loginPage{Error: msg})
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
	return h.html(c, "mfa.html", mfaPage{})
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
		msg := "Invalid code"
		return h.html(c, "mfa.html", mfaPage{Error: msg})
	}
	claims, err := h.Signer.ParseAccessToken(tokens.AccessToken)
	if err != nil {
		return h.html(c, "mfa.html", mfaPage{Error: "Session error"})
	}
	sess.Delete(sessChallengeID)
	sessSetUUID(sess, sessAdminID, claims.AdminID)
	sess.Set(sessAccessToken, tokens.AccessToken)
	if err := sess.Save(); err != nil {
		return err
	}
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
		LayoutData:    h.shell("Dashboard", "dashboard", "dashboard-content", true),
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
