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

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/importer"
	"busca-cnpj-2026/internal/loader"
	"busca-cnpj-2026/internal/metrics"
)

func main() {
	if err := run(); err != nil {
		log.Fatal(err)
	}
}

func run() error {
	dataPath := flag.String("data-path", "./data", "Directory with Receita Federal CSV files")
	samplePercent := flag.Int("sample-percent", 100, "Percent of each file to import (1-100)")
	batchSize := flag.Int("batch-size", 100000, "COPY batch size")
	workers := flag.Int("workers", 10, "Parallel file workers")
	tune := flag.Bool("tune", false, "Enable PostgreSQL bulk-import session tuning")
	benchmark := flag.Bool("benchmark", false, "Print BENCHMARK summary line")
	profile := flag.Bool("profile", false, "Print parse vs COPY timing breakdown")
	skipRefs := flag.Bool("skip-refs", false, "Skip reference tables")
	skipEmpresas := flag.Bool("skip-empresas", false, "Skip empresas")
	skipEstab := flag.Bool("skip-estabelecimentos", false, "Skip estabelecimentos")
	skipSocios := flag.Bool("skip-socios", false, "Skip socios")
	skipSimples := flag.Bool("skip-simples", false, "Skip simples")
	noClean := flag.Bool("no-clean", false, "Do not truncate tables before import")
	flag.Parse()

	if err := config.Load(); err != nil {
		return err
	}

	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()

	copier, err := loader.NewPGCopier(ctx, config.GetDSN(), *tune)
	if err != nil {
		return fmt.Errorf("pg copier: %w", err)
	}
	defer copier.Close()

	logger := log.New(os.Stdout, "", log.LstdFlags)
	collector := metrics.NewCollector("import")
	reporterCtx, cancelReporter := context.WithCancel(ctx)
	defer cancelReporter()
	go collector.StartReporter(reporterCtx, 10*time.Second, logger)

	opts := importer.Options{
		DataPath:             *dataPath,
		SamplePercent:        *samplePercent,
		BatchSize:            *batchSize,
		Workers:              *workers,
		Tune:                 *tune,
		Benchmark:            *benchmark,
		Profile:              *profile,
		SkipRefs:             *skipRefs,
		SkipEmpresas:         *skipEmpresas,
		SkipEstabelecimentos: *skipEstab,
		SkipSocios:           *skipSocios,
		SkipSimples:          *skipSimples,
		NoClean:              *noClean,
	}

	logger.Printf("import started sample=%d%% path=%s", opts.SamplePercent, opts.DataPath)
	runner := importer.Runner{Opts: opts, Copier: copier, Logger: logger, Metrics: collector}
	if _, runErr := runner.Run(ctx); runErr != nil {
		return fmt.Errorf("import: %w", runErr)
	}
	return nil
}
