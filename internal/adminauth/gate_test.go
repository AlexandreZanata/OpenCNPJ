package adminauth_test

import (
	"bytes"
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/base64"
	"encoding/json"
	"encoding/pem"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"github.com/alicebob/miniredis/v2"
	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/bruteforce"
	"busca-cnpj-2026/internal/adminauth/challenge"
	"busca-cnpj-2026/internal/adminauth/cipher"
	adminhandlers "busca-cnpj-2026/internal/adminauth/handlers"
	"busca-cnpj-2026/internal/adminauth/password"
	"busca-cnpj-2026/internal/adminauth/token"
	totpsvc "busca-cnpj-2026/internal/adminauth/totp"
	"busca-cnpj-2026/internal/adminauth/usecase"
)

type memRepo struct {
	admin     adminauth.AdminRow
	encSecret []byte
	refresh   map[string]uuid.UUID
}

func (m *memRepo) GetByEmail(_ context.Context, email string) (adminauth.AdminRow, error) {
	if m.admin.Email != email {
		return adminauth.AdminRow{}, errNoRows{}
	}
	return m.admin, nil
}

func (m *memRepo) LoadMFASecret(_ context.Context, id uuid.UUID) ([]byte, error) {
	if m.admin.ID != id {
		return nil, errNoRows{}
	}
	return m.encSecret, nil
}

func (m *memRepo) StoreRefreshToken(_ context.Context, _ uuid.UUID, token string, _ time.Time) error {
	if m.refresh == nil {
		m.refresh = map[string]uuid.UUID{}
	}
	m.refresh[token] = uuid.New()
	return nil
}

func (m *memRepo) FindRefreshToken(_ context.Context, token string) (uuid.UUID, uuid.UUID, error) {
	if _, ok := m.refresh[token]; !ok {
		return uuid.Nil, uuid.Nil, errNoRows{}
	}
	return uuid.New(), m.admin.ID, nil
}

func (m *memRepo) RevokeRefreshToken(context.Context, uuid.UUID) error { return nil }

type errNoRows struct{}

func (errNoRows) Error() string { return "no rows" }

// Gate tests from Phase 5 checklist (login → MFA → JWT → protected route).
func TestPhase5Gate(t *testing.T) {
	ctx := context.Background()
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})

	plainSecret := "JBSWY3DPEHPK3PXP"
	aead, err := cipher.NewAESGCM(testAESKey())
	if err != nil {
		t.Fatal(err)
	}
	enc, err := aead.Encrypt([]byte(plainSecret))
	if err != nil {
		t.Fatal(err)
	}
	hash, err := password.HashBytes("admin-pass")
	if err != nil {
		t.Fatal(err)
	}
	adminID := uuid.New()
	repo := &memRepo{
		admin: adminauth.AdminRow{
			ID: adminID, Email: "admin@test.local",
			PasswordHash: hash, MFAEnabled: true,
		},
		encSecret: enc,
	}
	cfg := testAdminCfg(t)
	signer, err := token.NewRS256Signer(cfg.JWTPrivateKeyPath, cfg.JWTPublicKeyPath, 15*time.Minute, cfg.Role)
	if err != nil {
		t.Fatal(err)
	}
	guard := bruteforce.NewGuard(rdb, cfg.MaxLoginFailures, cfg.LockoutMinutes)
	chStore := challenge.NewStore(rdb, cfg.ChallengeTTLSeconds)
	totpSvc := totpsvc.NewService(cfg.TOTPIssuer)

	loginDeps := usecase.LoginDeps{Repo: repo, Guard: guard, ChStore: chStore, Cfg: cfg}
	verifyDeps := usecase.VerifyMFADeps{
		Repo: repo, ChStore: chStore, Cipher: aead, TOTP: totpSvc, Signer: signer, Cfg: cfg,
	}
	handler := adminhandlers.NewAuthHandler(
		func(c context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error) {
			return usecase.Login(c, loginDeps, in)
		},
		func(c context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error) {
			return usecase.VerifyMFA(c, verifyDeps, in)
		},
		nil,
		cfg.RefreshCookieName,
	)
	app := fiber.New()
	adminhandlers.RegisterRoutes(app, handler, signer, "")

	// 1. Login without MFA code returns mfa_required
	loginResp := postJSON(t, app, "/admin/api/v1/auth/login", `{"email":"admin@test.local","password":"admin-pass"}`)
	if loginResp.StatusCode != http.StatusOK {
		t.Fatalf("login status=%d", loginResp.StatusCode)
	}
	var loginBody map[string]any
	decodeJSON(t, loginResp, &loginBody)
	if loginBody["status"] != "mfa_required" || loginBody["challengeId"] == "" {
		t.Fatalf("expected mfa_required, got %+v", loginBody)
	}
	chID, _ := loginBody["challengeId"].(string)

	// 2. Wrong TOTP → 401
	bad := postJSON(t, app, "/admin/api/v1/auth/mfa/verify",
		`{"challengeId":"`+chID+`","code":"000000"}`)
	if bad.StatusCode != http.StatusUnauthorized {
		t.Fatalf("wrong totp status=%d", bad.StatusCode)
	}

	// Re-login for fresh challenge (previous consumed on wrong attempt? wrong attempt consumes challenge)
	loginResp = postJSON(t, app, "/admin/api/v1/auth/login", `{"email":"admin@test.local","password":"admin-pass"}`)
	decodeJSON(t, loginResp, &loginBody)
	chID, _ = loginBody["challengeId"].(string)
	code, err := totpSvc.CurrentCode(plainSecret)
	if err != nil {
		t.Fatal(err)
	}

	// 3. Valid TOTP → JWT + cookie
	verifyResp := postJSON(t, app, "/admin/api/v1/auth/mfa/verify",
		`{"challengeId":"`+chID+`","code":"`+code+`"}`)
	if verifyResp.StatusCode != http.StatusOK {
		t.Fatalf("verify status=%d body=%s", verifyResp.StatusCode, readBody(verifyResp))
	}
	var verifyBody map[string]any
	decodeJSON(t, verifyResp, &verifyBody)
	access, _ := verifyBody["accessToken"].(string)
	if access == "" {
		t.Fatal("missing accessToken")
	}
	cookie := verifyResp.Header.Get("Set-Cookie")
	if !strings.Contains(cookie, "opencnpj_admin_refresh") {
		t.Fatalf("missing refresh cookie: %s", cookie)
	}

	// 4. Protected route without token → 401
	meResp := httptest.NewRequest(http.MethodGet, "/admin/api/v1/me", http.NoBody)
	unauth, err := app.Test(meResp)
	if err != nil {
		t.Fatal(err)
	}
	if unauth.StatusCode != http.StatusUnauthorized {
		t.Fatalf("unauth me status=%d", unauth.StatusCode)
	}

	// Protected route with token → 200
	req := httptest.NewRequest(http.MethodGet, "/admin/api/v1/me", http.NoBody)
	req.Header.Set("Authorization", "Bearer "+access)
	authMe, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	if authMe.StatusCode != http.StatusOK {
		t.Fatalf("auth me status=%d", authMe.StatusCode)
	}
	_ = ctx
}

func testAdminCfg(t *testing.T) adminauth.Config {
	t.Helper()
	dir := t.TempDir()
	priv := filepath.Join(dir, "priv.pem")
	pub := filepath.Join(dir, "pub.pem")
	writeRSAKeys(t, priv, pub)
	t.Setenv("MFA_SECRET_ENCRYPTION_KEY", base64.StdEncoding.EncodeToString(testAESKey()))
	t.Setenv("ADMIN_JWT_PRIVATE_KEY_PATH", priv)
	t.Setenv("ADMIN_JWT_PUBLIC_KEY_PATH", pub)
	cfg, err := adminauth.LoadConfig(15, 30, "OpenCNPJ-Admin")
	if err != nil {
		t.Fatal(err)
	}
	return cfg
}

func testAESKey() []byte { return []byte("01234567890123456789012345678901") }

func writeRSAKeys(t *testing.T, privPath, pubPath string) {
	t.Helper()
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		t.Fatal(err)
	}
	privDER := x509.MarshalPKCS1PrivateKey(key)
	if err := os.WriteFile(privPath, pem.EncodeToMemory(&pem.Block{
		Type: "RSA PRIVATE KEY", Bytes: privDER,
	}), 0o600); err != nil {
		t.Fatal(err)
	}
	pubDER, err := x509.MarshalPKIXPublicKey(&key.PublicKey)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(pubPath, pem.EncodeToMemory(&pem.Block{
		Type: "PUBLIC KEY", Bytes: pubDER,
	}), 0o644); err != nil {
		t.Fatal(err)
	}
}

func postJSON(t *testing.T, app *fiber.App, path, body string) *http.Response {
	t.Helper()
	req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	return resp
}

func decodeJSON(t *testing.T, resp *http.Response, out any) {
	t.Helper()
	defer resp.Body.Close()
	if err := json.NewDecoder(resp.Body).Decode(out); err != nil {
		t.Fatal(err)
	}
}

func readBody(resp *http.Response) string {
	b, _ := io.ReadAll(resp.Body)
	return string(b)
}

// Ensure memRepo satisfies interfaces at compile time.
var (
	_ interface {
		GetByEmail(context.Context, string) (adminauth.AdminRow, error)
	} = (*memRepo)(nil)
)

// Silence unused import.
var _ = bytes.NewBuffer(nil)
