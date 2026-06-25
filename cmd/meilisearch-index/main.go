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
	maxBatches := flag.Int("max-batches", 0, "Max batches per index stream (0 = all)")
	flag.Parse()

	if err := config.Load(); err != nil {
		log.Fatal(err)
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
	if err := client.Health(context.Background()); err != nil {
		log.Fatalf("meilisearch health: %v", err)
	}
	idx := meilisearch.NewIndexer(client, database.DB, log.New(os.Stdout, "", log.LstdFlags))
	opts := meilisearch.SyncOptions{
		BatchSize:             *batchSize,
		MaxBatches:            *maxBatches,
		SelectiveActiveMatriz: cfg.SelectiveActiveMatriz,
	}
	if err := idx.SyncAll(context.Background(), opts); err != nil {
		log.Fatal(err)
	}
	log.Println("meilisearch sync complete")
}
