package main

import (
	"flag"
	"log"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
)

func main() {
	saas := flag.Bool("saas", false, "run SaaS metadata migrations (opencnpj_saas only)")
	flag.Parse()

	if err := run(*saas); err != nil {
		log.Fatal(err)
	}
}

func run(saas bool) error {
	if err := config.Load(); err != nil {
		return err
	}
	if saas {
		if err := database.InitPostgresForSaasMigrate(); err != nil {
			return err
		}
		defer func() { _ = database.ClosePostgres() }()
		if err := database.RunSaasMigrations(); err != nil {
			return err
		}
		log.Println("saas migrations applied")
		return nil
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
