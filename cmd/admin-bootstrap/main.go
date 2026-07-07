package main

import (
	"bufio"
	"context"
	"flag"
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"golang.org/x/term"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/cipher"
	"busca-cnpj-2026/internal/adminauth/password"
	totpsvc "busca-cnpj-2026/internal/adminauth/totp"
	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
	saasdb "busca-cnpj-2026/internal/db/saas"
)

func main() {
	email := flag.String("email", "", "admin email (required)")
	flag.Parse()
	if strings.TrimSpace(*email) == "" {
		log.Fatal("--email is required")
	}
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

	pass, err := readPassword("New admin password: ")
	if err != nil {
		log.Fatalf("password: %v", err)
	}
	confirm, err := readPassword("Confirm password: ")
	if err != nil {
		log.Fatalf("password: %v", err)
	}
	if pass != confirm {
		log.Fatal("passwords do not match")
	}
	if len(pass) < 12 {
		log.Fatal("password must be at least 12 characters")
	}

	hash, err := password.HashBytes(pass)
	if err != nil {
		log.Fatalf("hash: %v", err)
	}

	totp := totpsvc.NewService(cfg.TOTPIssuer)
	secret, otpURL, err := totp.Generate(strings.TrimSpace(*email))
	if err != nil {
		log.Fatalf("totp: %v", err)
	}
	enc, err := aead.Encrypt([]byte(secret))
	if err != nil {
		log.Fatalf("encrypt: %v", err)
	}

	ctx, cancel := context.WithTimeout(context.Background(), 15*time.Second)
	defer cancel()
	repo := adminauth.NewAdminRepository(saasdb.New(database.SaaSPool))
	row, err := repo.UpsertAdmin(ctx, strings.TrimSpace(*email), hash, true)
	if err != nil {
		log.Fatalf("upsert admin: %v", err)
	}
	if err := repo.SaveMFASecret(ctx, row.ID, enc); err != nil {
		log.Fatalf("save mfa secret: %v", err)
	}

	fmt.Println("Admin user provisioned.")
	fmt.Println("Scan this TOTP URL once (store secret offline):")
	fmt.Println(otpURL)
	fmt.Println("Base32 secret (backup):", secret)
}

func readPassword(prompt string) (string, error) {
	fmt.Fprint(os.Stderr, prompt)
	if term.IsTerminal(int(os.Stdin.Fd())) {
		b, err := term.ReadPassword(int(os.Stdin.Fd()))
		fmt.Fprintln(os.Stderr)
		if err != nil {
			return "", err
		}
		return string(b), nil
	}
	sc := bufio.NewScanner(os.Stdin)
	if !sc.Scan() {
		return "", sc.Err()
	}
	return sc.Text(), nil
}
