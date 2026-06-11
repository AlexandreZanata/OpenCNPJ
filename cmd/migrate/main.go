package main

import (
	"log"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
)

func main() {
	if err := config.Load(); err != nil {
		log.Fatalf("config: %v", err)
	}
	if err := database.InitPostgres(); err != nil {
		log.Fatalf("postgres: %v", err)
	}
	defer database.ClosePostgres()
	if err := database.RunMigrations(); err != nil {
		log.Fatalf("migrate: %v", err)
	}
	log.Println("migrations applied")
}
