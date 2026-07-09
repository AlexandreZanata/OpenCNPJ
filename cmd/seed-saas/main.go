package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/cipher"
	"busca-cnpj-2026/internal/adminauth/password"
	totpsvc "busca-cnpj-2026/internal/adminauth/totp"
	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

func main() {
	email := strings.TrimSpace(envOr("ADMIN_EMAIL", "admin@comerc.app.br"))
	pass := envOr("ADMIN_PASSWORD", "")
	if len(pass) < 12 {
		log.Fatal("ADMIN_PASSWORD must be at least 12 characters")
	}
	clientName := envOr("API_CLIENT_NAME", "Comerc Default")
	clientEmail := envOr("API_CLIENT_EMAIL", "api@comerc.app.br")

	if err := config.Load(); err != nil {
		log.Fatalf("config: %v", err)
	}
	if err := database.InitSaaSPgx(); err != nil {
		log.Fatalf("saas db: %v", err)
	}
	defer database.CloseSaaSPgx()

	cfg, err := adminauth.LoadConfig(
		config.AppConfig.SaaS.AdminJWTTTLMinutes,
		config.AppConfig.SaaS.AdminRefreshTTLDays,
		config.AppConfig.SaaS.MFATOTPIssuer,
	)
	if err != nil {
		log.Fatalf("admin config: %v", err)
	}
	aead, err := cipher.NewAESGCM(cfg.MFASecretKey)
	if err != nil {
		log.Fatalf("cipher: %v", err)
	}

	hash, err := password.HashBytes(pass)
	if err != nil {
		log.Fatalf("hash: %v", err)
	}
	totp := totpsvc.NewService(cfg.TOTPIssuer)
	secret, otpURL, err := totp.Generate(email)
	if err != nil {
		log.Fatalf("totp: %v", err)
	}
	enc, err := aead.Encrypt([]byte(secret))
	if err != nil {
		log.Fatalf("encrypt: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	queries := saasdb.New(database.SaaSPool)
	repo := adminauth.NewAdminRepository(queries)
	row, err := repo.UpsertAdmin(ctx, email, hash, true)
	if err != nil {
		log.Fatalf("upsert admin: %v", err)
	}
	if err := repo.SaveMFASecret(ctx, row.ID, enc); err != nil {
		log.Fatalf("save mfa: %v", err)
	}

	plainKey, _, err := saas.CreateClientKey(ctx, queries, clientName, clientEmail, "production", 60, 0)
	if err != nil {
		log.Fatalf("api key: %v", err)
	}

	fmt.Printf("ADMIN_EMAIL=%s\n", email)
	fmt.Printf("ADMIN_PASSWORD=%s\n", pass)
	fmt.Printf("TOTP_URL=%s\n", otpURL)
	fmt.Printf("TOTP_SECRET=%s\n", secret)
	fmt.Printf("API_KEY=%s\n", plainKey)
}

func envOr(key, fallback string) string {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		return v
	}
	return fallback
}
