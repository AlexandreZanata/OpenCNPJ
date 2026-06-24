package pipeline

//nolint:misspell // Uses official Receita Federal field names.

import (
	"bufio"
	"context"
	"fmt"
	"os"
	"runtime"
	"sync"

	"busca-cnpj-2026/internal/loader"
	"busca-cnpj-2026/internal/metrics"
	"busca-cnpj-2026/internal/model"
	"busca-cnpj-2026/internal/parser"
)

const (
	channelBuffer = 10_000
	readerBuffer  = 4 * 1024 * 1024
)

type EmpresaPipeline struct {
	Copier  loader.BatchInserter
	Lookups *parser.LookupStore
	Metrics *metrics.Collector
}

func (p *EmpresaPipeline) Run(ctx context.Context, filePath string) error {
	// #nosec G304 -- filePath comes from a trusted ingestion manifest.
	f, err := os.Open(filePath)
	if err != nil {
		return fmt.Errorf("open file: %w", err)
	}
	defer f.Close()

	reader := parser.NewCSVReader(bufio.NewReaderSize(f, readerBuffer))
	rawCh := make(chan []string, channelBuffer)
	modelCh := make(chan model.Empresa, channelBuffer)
	errCh := make(chan error, runtime.NumCPU()+2)

	var wg sync.WaitGroup

	wg.Add(1)
	go func() {
		defer wg.Done()
		parser.ReadRawLines(ctx, reader, rawCh, errCh)
	}()

	parseWorkers := runtime.NumCPU()
	if parseWorkers < 1 {
		parseWorkers = 1
	}
	var parseWG sync.WaitGroup
	parseWG.Add(parseWorkers)
	for i := 0; i < parseWorkers; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			defer parseWG.Done()
			for line := range rawCh {
				if p.Metrics != nil {
					p.Metrics.AddBytes(metrics.CSVRecordBytes(line))
				}
				empresa, parseErr := parser.ParseEmpresa(line, p.Lookups)
				if parseErr != nil {
					if p.Metrics != nil {
						p.Metrics.AddError()
					}
					errCh <- parseErr
					continue
				}
				modelCh <- empresa
			}
		}()
	}

	wg.Add(1)
	go func() {
		defer wg.Done()
		parseWG.Wait()
		close(modelCh)
	}()

	wg.Add(1)
	go func() {
		defer wg.Done()
		batcher := loader.NewBatcher(5_000)
		columns := []string{
			"cnpj_basico", "razao_social", "natureza_juridica", "qualificacao_responsavel",
			"capital_social", "porte_empresa", "ente_federativo_responsavel",
		}
		for m := range modelCh {
			row := []any{
				m.CNPJBasico, m.RazaoSocial, m.NaturezaJuridica, m.QualificacaoResponsavel,
				m.CapitalSocial, m.PorteEmpresa, m.EnteFederativoResponsavel,
			}
			if rows, flush := batcher.Add(row); flush {
				if _, copyErr := p.Copier.CopyRows(ctx, "public", "empresas", columns, rows); copyErr != nil {
					errCh <- fmt.Errorf("copy empresas: %w", copyErr)
					return
				}
				if p.Metrics != nil {
					p.Metrics.AddRows(int64(len(rows)))
				}
			}
		}

		if rows := batcher.Flush(); len(rows) > 0 {
			if _, copyErr := p.Copier.CopyRows(ctx, "public", "empresas", columns, rows); copyErr != nil {
				errCh <- fmt.Errorf("copy empresas flush: %w", copyErr)
				return
			}
			if p.Metrics != nil {
				p.Metrics.AddRows(int64(len(rows)))
			}
		}
	}()

	wg.Wait()
	close(errCh)

	for runErr := range errCh {
		if runErr != nil {
			return runErr
		}
	}
	return nil
}
