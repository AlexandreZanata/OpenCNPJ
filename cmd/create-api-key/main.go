package main

import (
	"context"
	"fmt"
	"log"
	"os"
	"strings"
	"time"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

func main() {
	name := envOr("API_CLIENT_NAME", "Test Integration")
	email := envOr("API_CLIENT_EMAIL", "test-integration@comerc.app.br")
	label := envOr("API_KEY_LABEL", "test")

	if err := config.Load(); err != nil {
		log.Fatalf("config: %v", err)
	}
	if err := database.InitSaaSPgx(); err != nil {
		log.Fatalf("saas db: %v", err)
	}
	defer database.CloseSaaSPgx()

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	plainKey, clientID, err := saas.CreateClientKey(
		ctx, saasdb.New(database.SaaSPool), name, email, label, 60, 0,
	)
	if err != nil {
		log.Fatalf("create key: %v", err)
	}

	fmt.Printf("API_CLIENT_NAME=%s\n", name)
	fmt.Printf("API_CLIENT_EMAIL=%s\n", email)
	fmt.Printf("API_CLIENT_ID=%s\n", clientID)
	fmt.Printf("API_KEY=%s\n", plainKey)
}

func envOr(key, fallback string) string {
	if v := strings.TrimSpace(os.Getenv(key)); v != "" {
		return v
	}
	return fallback
}
