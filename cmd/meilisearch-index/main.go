package main

import (
	"context"
	"flag"
	"log"
	"os"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/meilisearch"
)

func main() {
	batchSize := flag.Int("batch-size", 5000, "Documents per Meilisearch batch")
	flag.Parse()

	if err := config.Load(); err != nil {
		log.Fatal(err)
	}
	if !config.AppConfig.Meilisearch.Enabled {
		log.Fatal("meilisearch.enabled must be true in config")
	}
	if err := database.InitPostgresForMigrate(); err != nil {
		log.Fatal(err)
	}
	defer func() {
		if err := database.ClosePostgres(); err != nil {
			log.Printf("Warning: failed to close PostgreSQL: %v", err)
		}
	}()

	cfg := config.AppConfig.Meilisearch
	client := meilisearch.NewClient(cfg.Host, cfg.Port, cfg.APIKey)
	idx := meilisearch.NewIndexer(client, database.DB, log.New(os.Stdout, "", log.LstdFlags))
	if err := idx.SyncAll(context.Background(), *batchSize); err != nil {
		log.Fatal(err)
	}
	log.Println("meilisearch sync complete")
}
