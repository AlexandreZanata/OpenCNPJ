package main

import (
	"log"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
)

func main() {
	if err := run(); err != nil {
		log.Fatal(err)
	}
}

func run() error {
	if err := config.Load(); err != nil {
		return err
	}
	if err := database.InitPostgresForMigrate(); err != nil {
		return err
	}
	defer func() { _ = database.ClosePostgres() }()
	if err := database.RunMigrations(); err != nil {
		return err
	}
	log.Println("migrations applied")
	return nil
}
