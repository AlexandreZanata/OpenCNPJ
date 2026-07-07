package adminauth

import (
	"encoding/base64"
	"os"
	"path/filepath"
	"testing"
)

func TestLoadConfig(t *testing.T) {
	dir := t.TempDir()
	priv := filepath.Join(dir, "priv.pem")
	pub := filepath.Join(dir, "pub.pem")
	if err := os.WriteFile(priv, []byte("x"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(pub, []byte("y"), 0o644); err != nil {
		t.Fatal(err)
	}
	key := base64.StdEncoding.EncodeToString([]byte("01234567890123456789012345678901"))
	t.Setenv("MFA_SECRET_ENCRYPTION_KEY", key)
	t.Setenv("ADMIN_JWT_PRIVATE_KEY_PATH", priv)
	t.Setenv("ADMIN_JWT_PUBLIC_KEY_PATH", pub)
	cfg, err := LoadConfig(15, 30, "OpenCNPJ-Admin")
	if err != nil {
		t.Fatal(err)
	}
	if cfg.AccessTTLMinutes != 15 || cfg.RefreshTTLDays != 30 {
		t.Fatalf("ttl mismatch: %+v", cfg)
	}
}

func TestLoadConfigRejectsShortKey(t *testing.T) {
	t.Setenv("MFA_SECRET_ENCRYPTION_KEY", base64.StdEncoding.EncodeToString([]byte("short")))
	t.Setenv("ADMIN_JWT_PRIVATE_KEY_PATH", "/tmp/x")
	t.Setenv("ADMIN_JWT_PUBLIC_KEY_PATH", "/tmp/y")
	if _, err := LoadConfig(15, 30, ""); err == nil {
		t.Fatal("expected error for short key")
	}
}
