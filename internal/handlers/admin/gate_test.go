package admin_test

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"io"
	"net/http"
	"net/http/httptest"
	"net/url"
	"os"
	"strings"
	"testing"
	"time"

	"github.com/alicebob/miniredis/v2"
	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/session"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgtype"
	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/adminauth/token"
	"busca-cnpj-2026/internal/adminauth/usecase"
	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/handlers/admin"
	"busca-cnpj-2026/internal/saas"
)

// Phase 6 gate: HTML login flow, client+key create, usage page, static CSS.
func TestPhase6Gate(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	_ = redis.NewClient(&redis.Options{Addr: mr.Addr()})

	signer := testSigner(t)
	adminID := uuid.New()
	challengeID := uuid.New()
	clientID := uuid.New()
	keyID := uuid.New()

	q := &mockQuerier{
		clientID: clientID,
		keyID:    keyID,
		usage: []saasdb.ListRecentUsageRow{{
			ClientID: pgUUID(clientID), ClientName: "Gate Co",
			Date: pgDate(time.Now()), RequestCount: 5, CnpjLookupCount: 2,
		}},
	}

	store := session.New(session.Config{
		KeyLookup: "cookie:opencnpj_admin_session", Expiration: time.Hour,
	})
	h := admin.NewHandler(admin.Deps{
		Queries: q,
		Login: func(_ context.Context, in usecase.LoginInput) (usecase.LoginMFARequired, error) {
			if in.Email == "admin@test.local" && in.Password == "pass" {
				return usecase.LoginMFARequired{ChallengeID: challengeID, ExpiresInSeconds: 300}, nil
			}
			return usecase.LoginMFARequired{}, errBadCreds{}
		},
		Verify: func(_ context.Context, in usecase.VerifyMFAInput) (usecase.AuthTokens, error) {
			if in.ChallengeID == challengeID && in.Code == "123456" {
				tok, _, _ := signer.SignAccessToken(adminID, true)
				return usecase.AuthTokens{
					AccessToken: tok, ExpiresInSeconds: 900,
					RefreshToken: "refresh", RefreshExpires: time.Now().Add(24 * time.Hour),
				}, nil
			}
			return usecase.AuthTokens{}, errBadMFA{}
		},
		Signer: signer, Session: store, RefreshCookie: "opencnpj_admin_refresh",
		DefaultRate: 60, DefaultQuota: 0, Renderer: admin.MustRenderer(),
	})

	app := fiber.New()
	if err := admin.RegisterRoutes(app, h, ""); err != nil {
		t.Fatal(err)
	}

	loginPage := get(t, app, "/admin/login")
	csrf := extractCSRF(readBody(loginPage))

	// Login → MFA redirect
	loginResp := postForm(t, app, "/admin/login", url.Values{
		"email": {"admin@test.local"}, "password": {"pass"}, "_csrf": {csrf},
	}, loginPage.Cookies()...)
	if loginResp.StatusCode != http.StatusFound {
		t.Fatalf("login status=%d", loginResp.StatusCode)
	}
	if !strings.Contains(loginResp.Header.Get("Location"), "/admin/mfa") {
		t.Fatalf("expected mfa redirect")
	}

	// MFA → dashboard
	mfaPage := get(t, app, "/admin/mfa", loginResp.Cookies()...)
	mfaCSRF := extractCSRF(readBody(mfaPage))
	mfaResp := postForm(t, app, "/admin/mfa", url.Values{"code": {"123456"}, "_csrf": {mfaCSRF}}, mergeCookies(loginPage.Cookies(), loginResp.Cookies())...)
	if mfaResp.StatusCode != http.StatusFound {
		t.Fatalf("mfa status=%d body=%s", mfaResp.StatusCode, readBody(mfaResp))
	}

	cookies := mergeCookies(loginPage.Cookies(), loginResp.Cookies(), mfaResp.Cookies())
	dash := get(t, app, "/admin/", cookies...)
	dashBody := readBody(dash)
	if dash.StatusCode != http.StatusOK || !strings.Contains(dashBody, "Dashboard") {
		t.Fatalf("dashboard failed status=%d body=%s", dash.StatusCode, dashBody)
	}
	dashCSRF := extractCSRF(dashBody)

	// Static CSS
	staticResp := get(t, app, "/admin/static/admin.css", cookies...)
	if staticResp.StatusCode != http.StatusOK {
		t.Fatalf("static css status=%d", staticResp.StatusCode)
	}

	// Create client
	create := postForm(t, app, "/admin/clients", url.Values{
		"name": {"Gate Co"}, "email": {"gate@test.local"},
		"rate_limit": {"60"}, "monthly_quota": {"0"}, "_csrf": {dashCSRF},
	}, cookies...)
	if create.StatusCode != http.StatusFound {
		t.Fatalf("create client status=%d", create.StatusCode)
	}

	// Generate key — one-time plaintext
	detailPage := get(t, app, "/admin/clients/"+clientID.String(), cookies...)
	detailCSRF := extractCSRF(readBody(detailPage))
	keyResp := postForm(t, app, "/admin/clients/"+clientID.String()+"/keys", url.Values{
		"label": {"production"}, "_csrf": {detailCSRF},
	}, cookies...)
	if keyResp.StatusCode != http.StatusFound {
		t.Fatalf("create key status=%d", keyResp.StatusCode)
	}
	detail := get(t, app, "/admin/clients/"+clientID.String(), cookies...)
	body := readBody(detail)
	if !strings.Contains(body, "Copy now") || !strings.Contains(body, "ocnpj_live_") {
		t.Fatalf("expected one-time key in detail page")
	}

	// Usage page
	usage := get(t, app, "/admin/usage", cookies...)
	if usage.StatusCode != http.StatusOK || !strings.Contains(readBody(usage), "Gate Co") {
		t.Fatalf("usage page failed")
	}

	// Unauthenticated → redirect login
	unauth := get(t, app, "/admin/clients")
	if unauth.StatusCode != http.StatusFound {
		t.Fatalf("unauth should redirect, got %d", unauth.StatusCode)
	}
}

type errBadCreds struct{}

func (errBadCreds) Error() string { return "bad creds" }

type errBadMFA struct{}

func (errBadMFA) Error() string { return "bad mfa" }

type mockQuerier struct {
	clientID uuid.UUID
	keyID    uuid.UUID
	usage    []saasdb.ListRecentUsageRow
}

func (m *mockQuerier) CountAPIClients(context.Context) (int64, error)       { return 1, nil }
func (m *mockQuerier) SumUsageRequestsToday(context.Context) (int64, error) { return 5, nil }
func (m *mockQuerier) ListAPIClients(context.Context, saasdb.ListAPIClientsParams) ([]saasdb.ApiClient, error) {
	return nil, nil
}
func (m *mockQuerier) GetClientByID(context.Context, pgtype.UUID) (saasdb.ApiClient, error) {
	return saasdb.ApiClient{
		ID: pgUUID(m.clientID), Name: "Gate Co", Email: "gate@test.local", Status: saas.ClientStatusActive,
	}, nil
}
func (m *mockQuerier) InsertAPIClient(context.Context, saasdb.InsertAPIClientParams) (saasdb.ApiClient, error) {
	return saasdb.ApiClient{ID: pgUUID(m.clientID), Name: "Gate Co", Status: saas.ClientStatusActive}, nil
}
func (m *mockQuerier) UpdateClientStatus(context.Context, saasdb.UpdateClientStatusParams) error {
	return nil
}
func (m *mockQuerier) ListAPIKeysByClient(context.Context, pgtype.UUID) ([]saasdb.ListAPIKeysByClientRow, error) {
	return []saasdb.ListAPIKeysByClientRow{{
		ID: pgUUID(m.keyID), KeyPrefix: "ocnpj_abcd1234", Label: "production",
		CreatedAt: pgtype.Timestamptz{Time: time.Now(), Valid: true},
	}}, nil
}
func (m *mockQuerier) InsertAPIKey(ctx context.Context, arg saasdb.InsertAPIKeyParams) (saasdb.InsertAPIKeyRow, error) {
	return saasdb.InsertAPIKeyRow{ID: pgUUID(m.keyID), KeyPrefix: arg.KeyPrefix, Label: arg.Label}, nil
}
func (m *mockQuerier) RevokeAPIKey(context.Context, saasdb.RevokeAPIKeyParams) (int64, error) {
	return 1, nil
}
func (m *mockQuerier) ListUsageByClient(context.Context, saasdb.ListUsageByClientParams) ([]saasdb.ApiUsageDaily, error) {
	return []saasdb.ApiUsageDaily{{
		ClientID: pgUUID(m.clientID), Date: pgDate(time.Now()),
		RequestCount: 5, CnpjLookupCount: 2,
	}}, nil
}
func (m *mockQuerier) ListRecentUsage(context.Context, int32) ([]saasdb.ListRecentUsageRow, error) {
	return m.usage, nil
}

// Stubs for unused querier methods (admin auth SQL not used in panel gate).
func (m *mockQuerier) GetAPIKeyByHash(context.Context, []byte) (saasdb.GetAPIKeyByHashRow, error) {
	panic("unused")
}
func (m *mockQuerier) GetAdminMFASecret(context.Context, pgtype.UUID) (saasdb.AdminMfaSecret, error) {
	panic("unused")
}
func (m *mockQuerier) GetAdminUserByEmail(context.Context, string) (saasdb.AdminUser, error) {
	panic("unused")
}
func (m *mockQuerier) GetUsageDaily(context.Context, saasdb.GetUsageDailyParams) (saasdb.ApiUsageDaily, error) {
	panic("unused")
}
func (m *mockQuerier) GetValidRefreshToken(context.Context, []byte) (saasdb.AdminRefreshToken, error) {
	panic("unused")
}
func (m *mockQuerier) InsertAdminRefreshToken(context.Context, saasdb.InsertAdminRefreshTokenParams) (saasdb.AdminRefreshToken, error) {
	panic("unused")
}
func (m *mockQuerier) RevokeRefreshToken(context.Context, pgtype.UUID) error { panic("unused") }
func (m *mockQuerier) SetAdminMFAEnabled(context.Context, saasdb.SetAdminMFAEnabledParams) error {
	panic("unused")
}
func (m *mockQuerier) UpsertAdminMFASecret(context.Context, saasdb.UpsertAdminMFASecretParams) error {
	panic("unused")
}
func (m *mockQuerier) UpsertAdminUser(context.Context, saasdb.UpsertAdminUserParams) (saasdb.AdminUser, error) {
	panic("unused")
}
func (m *mockQuerier) UpsertUsageDaily(context.Context, saasdb.UpsertUsageDailyParams) error {
	panic("unused")
}
func (m *mockQuerier) InsertAdminAuditLog(context.Context, saasdb.InsertAdminAuditLogParams) (saasdb.AdminAuditLog, error) {
	return saasdb.AdminAuditLog{ID: 1}, nil
}

func pgUUID(id uuid.UUID) pgtype.UUID {
	return pgtype.UUID{Bytes: id, Valid: true}
}

func pgDate(t time.Time) pgtype.Date {
	return pgtype.Date{Time: t, Valid: true}
}

func get(t *testing.T, app *fiber.App, path string, cookies ...*http.Cookie) *http.Response {
	t.Helper()
	req := httptest.NewRequest(http.MethodGet, path, http.NoBody)
	for _, c := range cookies {
		req.AddCookie(c)
	}
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	return resp
}

func postForm(t *testing.T, app *fiber.App, path string, vals url.Values, cookies ...*http.Cookie) *http.Response {
	t.Helper()
	req := httptest.NewRequest(http.MethodPost, path, strings.NewReader(vals.Encode()))
	req.Header.Set("Content-Type", "application/x-www-form-urlencoded")
	for _, c := range cookies {
		req.AddCookie(c)
	}
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	return resp
}

func readBody(resp *http.Response) string {
	b, _ := io.ReadAll(resp.Body)
	_ = resp.Body.Close()
	return string(b)
}

func mergeCookies(groups ...[]*http.Cookie) []*http.Cookie {
	byName := map[string]*http.Cookie{}
	for _, g := range groups {
		for _, c := range g {
			byName[c.Name] = c
		}
	}
	out := make([]*http.Cookie, 0, len(byName))
	for _, c := range byName {
		out = append(out, c)
	}
	return out
}

func testSigner(t *testing.T) *token.RS256Signer {
	t.Helper()
	// Reuse gate test key helper pattern from adminauth — generate temp keys inline.
	dir := t.TempDir()
	priv := dir + "/priv.pem"
	pub := dir + "/pub.pem"
	writeRSAKeysFile(t, priv, pub)
	s, err := token.NewRS256Signer(priv, pub, 15*time.Minute, "super_admin")
	if err != nil {
		t.Fatal(err)
	}
	return s
}

func writeRSAKeysFile(t *testing.T, privPath, pubPath string) {
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
