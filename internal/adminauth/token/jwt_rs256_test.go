package token

import (
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"encoding/pem"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/google/uuid"
)

func TestRS256SignAndParse(t *testing.T) {
	dir := t.TempDir()
	privPath := filepath.Join(dir, "priv.pem")
	pubPath := filepath.Join(dir, "pub.pem")
	writeTestRSAKeys(t, privPath, pubPath)

	signer, err := NewRS256Signer(privPath, pubPath, 15*time.Minute, "super_admin")
	if err != nil {
		t.Fatal(err)
	}
	id := uuid.New()
	raw, ttl, err := signer.SignAccessToken(id, true)
	if err != nil || raw == "" || ttl != 900 {
		t.Fatalf("sign failed: ttl=%d err=%v", ttl, err)
	}
	claims, err := signer.ParseAccessToken(raw)
	if err != nil {
		t.Fatal(err)
	}
	if claims.AdminID != id || !claims.MFAVerified || claims.Role != "super_admin" {
		t.Fatalf("claims mismatch: %+v", claims)
	}
}

func writeTestRSAKeys(t *testing.T, privPath, pubPath string) {
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
