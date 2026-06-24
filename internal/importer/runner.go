package importer

import (
	"context"
	"fmt"
	"log"
	"sync"
	"sync/atomic"
	"time"

	"busca-cnpj-2026/internal/loader"
	"busca-cnpj-2026/internal/metrics"
	"busca-cnpj-2026/internal/parser"
)

type Result struct {
	TotalRows int64
	Elapsed   time.Duration
	Timings   *Timings
}

type Runner struct {
	Opts    Options
	Copier  loader.BatchInserter
	Logger  *log.Logger
	Metrics *metrics.Collector
}

func (r *Runner) Run(ctx context.Context) (Result, error) {
	started := time.Now()
	ds, err := DiscoverDataset(r.Opts.DataPath)
	if err != nil {
		return Result{}, err
	}

	if !r.Opts.SkipRefs {
		r.Logger.Printf("import: loading reference tables")
		if err := ImportReferences(ctx, ds, r.Copier); err != nil {
			return Result{}, err
		}
	}

	var timings Timings
	total, err := r.importStages(ctx, ds, &timings)
	if err != nil {
		return Result{}, err
	}

	elapsed := time.Since(started)
	if r.Opts.Benchmark {
		rps := float64(total) / elapsed.Seconds()
		r.Logger.Printf("BENCHMARK rows=%d rps=%.0f", total, rps)
	}
	if r.Opts.Profile {
		parseSec := float64(timings.ParseNs.Load()) / 1e9
		copySec := float64(timings.CopyNs.Load()) / 1e9
		workSec := parseSec + copySec
		parsePct, copyPct := 0.0, 0.0
		if workSec > 0 {
			parsePct = parseSec / workSec * 100
			copyPct = copySec / workSec * 100
		}
		r.Logger.Printf(
			"PROFILE wall_sec=%.2f parse_sec=%.2f copy_sec=%.2f parse=%.0f%% copy=%.0f%% (parallel)",
			elapsed.Seconds(), parseSec, copySec, parsePct, copyPct,
		)
	}

	r.Logger.Printf("import finished rows=%d elapsed=%s", total, elapsed.Round(time.Millisecond))
	return Result{TotalRows: total, Elapsed: elapsed, Timings: &timings}, nil
}

func (r *Runner) importStages(ctx context.Context, ds Dataset, timings *Timings) (int64, error) {
	var total int64

	if !r.Opts.SkipEmpresas {
		n, err := r.importGroup(ctx, ds.Empresas, FileJob{
			Table: "empresas",
			Columns: []string{
				"cnpj_basico", "razao_social", "natureza_juridica", "qualificacao_responsavel",
				"capital_social", "porte_empresa", "ente_federativo_responsavel",
			},
			MapRow: empresaRow, Label: "empresas",
		}, timings)
		if err != nil {
			return total, fmt.Errorf("empresas: %w", err)
		}
		total += n
	}

	var wg sync.WaitGroup
	errCh := make(chan error, 2)
	var estabRows atomic.Int64
	var socioRows atomic.Int64
	if !r.Opts.SkipEstabelecimentos {
		wg.Add(1)
		go func() {
			defer wg.Done()
			n, err := r.importGroup(ctx, ds.Estabelecimentos, FileJob{
				Table: "estabelecimentos",
				Columns: []string{
					"cnpj_basico", "cnpj_ordem", "cnpj_dv", "identificador_matriz_filial",
					"nome_fantasia", "situacao_cadastral", "data_situacao_cadastral", "motivo_situacao_cadastral",
					"nome_cidade_exterior", "pais", "data_inicio_atividade", "cnae_fiscal_principal",
					"cnae_fiscal_secundaria", "tipo_logradouro", "logradouro", "numero", "complemento",
					"bairro", "cep", "uf", "municipio", "ddd_1", "telefone_1", "ddd_2", "telefone_2",
					"ddd_fax", "fax", "email", "situacao_especial", "data_situacao_especial",
				},
				MapRow: estabelecimentoRow, Label: "estabelecimentos",
			}, timings)
			if err != nil {
				errCh <- fmt.Errorf("estabelecimentos: %w", err)
				return
			}
			estabRows.Add(n)
		}()
	}

	if !r.Opts.SkipSocios {
		wg.Add(1)
		go func() {
			defer wg.Done()
			n, err := r.importGroup(ctx, ds.Socios, FileJob{
				Table: "socios",
				Columns: []string{
					"cnpj_basico", "identificador_socio", "nome_socio", "cpf_cnpj_socio",
					"qualificacao_socio", "data_entrada_sociedade", "pais", "representante_legal",
					"nome_representante", "qualificacao_representante", "faixa_etaria",
				},
				MapRow: socioRow, Label: "socios",
			}, timings)
			if err != nil {
				errCh <- fmt.Errorf("socios: %w", err)
				return
			}
			socioRows.Add(n)
		}()
	}

	wg.Wait()
	close(errCh)
	for stageErr := range errCh {
		if stageErr != nil {
			return total, stageErr
		}
	}
	total += estabRows.Load() + socioRows.Load()

	if !r.Opts.SkipSimples && ds.Simples != "" {
		limit, err := RowLimit(ds.Simples, r.Opts.SamplePercent)
		if err != nil {
			return total, err
		}
		n, err := ImportFile(ctx, FileJob{
			Path: ds.Simples, Table: "simples",
			Columns: []string{
				"cnpj_basico", "opcao_simples", "data_opcao_simples", "data_exclusao_simples",
				"opcao_mei", "data_opcao_mei", "data_exclusao_mei",
			},
			Limit: limit, MapRow: func(line []string, _ *parser.LookupStore) ([]any, error) {
				return simplesRow(line)
			}, Label: "simples",
		}, r.Opts.BatchSize, r.Copier, nil, r.Metrics, timings, NewDedupeSet())
		if err != nil {
			return total, fmt.Errorf("simples: %w", err)
		}
		r.Logger.Printf("imported simples: %d rows", n)
		total += n
	}

	return total, nil
}

func (r *Runner) importGroup(
	ctx context.Context,
	files []string,
	template FileJob,
	timings *Timings,
) (int64, error) {
	sem := make(chan struct{}, maxWorkers(r.Opts.Workers))
	var wg sync.WaitGroup
	errCh := make(chan error, len(files))
	var total int64
	var mu sync.Mutex
	dedupe := NewDedupeSet()

	for _, path := range files {
		wg.Add(1)
		go func(filePath string) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			limit, err := RowLimit(filePath, r.Opts.SamplePercent)
			if err != nil {
				errCh <- err
				return
			}
			job := template
			job.Path = filePath
			job.Limit = limit

			n, importErr := ImportFile(ctx, job, r.Opts.BatchSize, r.Copier, nil, r.Metrics, timings, dedupe)
			if importErr != nil {
				errCh <- fmt.Errorf("%s %s: %w", template.Label, filePath, importErr)
				return
			}
			r.Logger.Printf("imported %s from %s: %d rows", template.Label, filePath, n)
			mu.Lock()
			total += n
			mu.Unlock()
		}(path)
	}

	wg.Wait()
	close(errCh)
	for groupErr := range errCh {
		if groupErr != nil {
			return total, groupErr
		}
	}
	return total, nil
}

func maxWorkers(workers int) int {
	if workers < 1 {
		return 1
	}
	return workers
}
