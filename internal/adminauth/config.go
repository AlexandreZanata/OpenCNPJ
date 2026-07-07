package adminauth

import (
	"fmt"
	"os"
	"strings"
)

// Config holds admin JWT, MFA, and cookie settings (env-backed).
type Config struct {
	JWTPrivateKeyPath    string
	JWTPublicKeyPath     string
	MFASecretKey         []byte
	TOTPIssuer           string
	RefreshCookieName    string
	AccessTTLMinutes     int
	RefreshTTLDays       int
	ChallengeTTLSeconds  int
	MaxLoginFailures     int
	LockoutMinutes         int
	Role                 string
}

// LoadConfig reads admin auth settings from environment variables.
func LoadConfig(accessTTLMin, refreshTTLDays int, totpIssuer string) (Config, error) {
	keyB64 := strings.TrimSpace(os.Getenv("MFA_SECRET_ENCRYPTION_KEY"))
	if keyB64 == "" {
		return Config{}, fmt.Errorf("MFA_SECRET_ENCRYPTION_KEY is required")
	}
	key, err := decodeBase64Key(keyB64)
	if err != nil {
		return Config{}, err
	}
	priv := strings.TrimSpace(os.Getenv("ADMIN_JWT_PRIVATE_KEY_PATH"))
	pub := strings.TrimSpace(os.Getenv("ADMIN_JWT_PUBLIC_KEY_PATH"))
	if priv == "" || pub == "" {
		return Config{}, fmt.Errorf("ADMIN_JWT_PRIVATE_KEY_PATH and ADMIN_JWT_PUBLIC_KEY_PATH are required")
	}
	cookie := strings.TrimSpace(os.Getenv("REFRESH_TOKEN_COOKIE_NAME"))
	if cookie == "" {
		cookie = "opencnpj_admin_refresh"
	}
	if accessTTLMin <= 0 {
		accessTTLMin = 15
	}
	if refreshTTLDays <= 0 {
		refreshTTLDays = 30
	}
	if totpIssuer == "" {
		totpIssuer = "OpenCNPJ-Admin"
	}
	return Config{
		JWTPrivateKeyPath:   priv,
		JWTPublicKeyPath:    pub,
		MFASecretKey:        key,
		TOTPIssuer:          totpIssuer,
		RefreshCookieName:   cookie,
		AccessTTLMinutes:    accessTTLMin,
		RefreshTTLDays:      refreshTTLDays,
		ChallengeTTLSeconds: 300,
		MaxLoginFailures:    5,
		LockoutMinutes:      15,
		Role:                "super_admin",
	}, nil
}
