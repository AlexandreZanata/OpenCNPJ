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
		outputDir     = flag.String("output", envOr("DATA_PATH", "./data"), "Output directory for CSV files")
		month         = flag.String("month", os.Getenv("DOWNLOAD_MONTH"), "Month as YYYY-MM (default: latest available)")
		baseURL       = flag.String("base-url", envOr("RFB_WEBDAV_URL", downloader.DefaultBaseURL), "Receita Federal WebDAV URL")
		shareToken    = flag.String("share-token", envOr("RFB_SHARE_TOKEN", downloader.DefaultShareToken), "Public share token")
		workers       = flag.Int("workers", 4, "Parallel downloads (reserved)")
		keepZIP       = flag.Bool("keep-zip", false, "Keep ZIP files after extraction")
		retryAttempts = flag.Int("retry", 3, "Download retry attempts per file")
		timeoutMin    = flag.Int("timeout", 30, "HTTP timeout in minutes")
		listOnly      = flag.Bool("list", false, "List available months only")
		noProgress    = flag.Bool("no-progress", false, "Disable terminal download progress bar")
	)
	flag.Parse()

	_ = workers // reserved for future parallel downloads

	client := downloader.NewClient(*baseURL, *shareToken, time.Duration(*timeoutMin)*time.Minute)

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	if *listOnly {
		months, err := client.ListMonthDirectories(ctx)
		if err != nil {
			log.Fatalf("list months failed: %v", err)
		}
		fmt.Println("Available months on Receita Federal:")
		for _, m := range months {
			fmt.Println(" ", m)
		}
		return
	}

	opts := downloader.Options{
		OutputDir:     *outputDir,
		Month:         *month,
		Workers:       *workers,
		KeepZIP:       *keepZIP,
		RetryAttempts: *retryAttempts,
	}

	var termProgress *downloader.TerminalProgress
	if !*noProgress {
		termProgress = downloader.NewTerminalProgress()
		opts.OnProgress = termProgress.Callback()
	}

	dl := downloader.NewDownloader(client, opts)

	result, err := dl.Run(ctx)
	if termProgress != nil {
		termProgress.Done()
	}
	if err != nil {
		log.Fatalf("download failed: %v", err)
	}

	fmt.Printf("\nDownload complete: month=%s files=%d downloaded=%d skipped=%d csvs=%d\n",
		result.Month, result.FilesTotal, result.FilesDownload, result.FilesSkipped, result.CSVExtracted)
}

func envOr(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}
