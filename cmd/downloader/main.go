package main

import (
	"context"
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"busca-cnpj-2026/internal/downloader"
)

func main() {
	var (
		outputDir     = flag.String("output", envOr("DATA_PATH", "./data"), "Diretório de saída dos CSVs")
		month         = flag.String("month", os.Getenv("DOWNLOAD_MONTH"), "Mês no formato YYYY-MM (padrão: mês atual ou mais recente)")
		baseURL       = flag.String("base-url", envOr("RFB_WEBDAV_URL", downloader.DefaultBaseURL), "URL WebDAV da Receita Federal")
		shareToken    = flag.String("share-token", envOr("RFB_SHARE_TOKEN", downloader.DefaultShareToken), "Token do compartilhamento público")
		workers       = flag.Int("workers", 4, "Downloads paralelos (reservado)")
		keepZIP       = flag.Bool("keep-zip", false, "Manter arquivos ZIP após extração")
		retryAttempts = flag.Int("retry", 3, "Tentativas de download por arquivo")
		timeoutMin    = flag.Int("timeout", 30, "Timeout HTTP em minutos")
		listOnly      = flag.Bool("list", false, "Apenas listar meses disponíveis")
	)
	flag.Parse()

	_ = workers // reservado para downloads paralelos futuros

	client := downloader.NewClient(*baseURL, *shareToken, time.Duration(*timeoutMin)*time.Minute)

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	if *listOnly {
		months, err := client.ListMonthDirectories(ctx)
		if err != nil {
			log.Fatalf("erro ao listar meses: %v", err)
		}
		fmt.Println("Meses disponíveis na Receita Federal:")
		for _, m := range months {
			fmt.Println(" ", m)
		}
		return
	}

	dl := downloader.NewDownloader(client, downloader.Options{
		OutputDir:     *outputDir,
		Month:         *month,
		Workers:       *workers,
		KeepZIP:       *keepZIP,
		RetryAttempts: *retryAttempts,
	})

	result, err := dl.Run(ctx)
	if err != nil {
		log.Fatalf("download falhou: %v", err)
	}

	fmt.Printf("\nDownload concluído: mês=%s baixados=%d ignorados=%d csvs=%d\n",
		result.Month, result.FilesDownload, result.FilesSkipped, result.CSVExtracted)
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
