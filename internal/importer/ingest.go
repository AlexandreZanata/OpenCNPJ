package importer

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"

	"busca-cnpj-2026/internal/loader"
	"busca-cnpj-2026/internal/parser"
)

const readBufSize = 4 * 1024 * 1024

type RowBuilder func(line []string, lookups *parser.LookupStore) ([]any, bool, error)

type IngestOpts struct {
	Copier        loader.BatchInserter
	BatchSize     int
	SamplePercent int
	Lookups       *parser.LookupStore
	FilterCNPJ    func(string) bool
	Dedupe        *DedupeSet
	DedupeKey     func(row []any) string
}

func IngestCSV(
	ctx context.Context,
	path, table string,
	columns []string,
	opts IngestOpts,
	build RowBuilder,
) (int64, error) {
	f, err := os.Open(path)
	if err != nil {
		return 0, fmt.Errorf("open %s: %w", path, err)
	}
	defer f.Close()

	reader := parser.NewCSVReader(bufio.NewReaderSize(f, readBufSize))
	batcher := loader.NewBatcher(opts.BatchSize)
	var total int64

	for {
		if err := ctx.Err(); err != nil {
			return total, err
		}
		line, err := reader.Read()
		if err == io.EOF {
			break
		}
		if err != nil {
			return total, fmt.Errorf("read %s: %w", path, err)
		}
		if opts.FilterCNPJ != nil && len(line) > 0 && !opts.FilterCNPJ(line[0]) {
			continue
		}
		row, ok, buildErr := build(line, opts.Lookups)
		if buildErr != nil || !ok {
			continue
		}
		if opts.Dedupe != nil && opts.DedupeKey != nil {
			if opts.Dedupe.Seen(opts.DedupeKey(row)) {
				continue
			}
		}
		if flushed, flush := batcher.Add(row); flush {
			n, copyErr := opts.Copier.CopyRows(ctx, "public", table, columns, flushed)
			if copyErr != nil {
				return total, copyErr
			}
			total += n
		}
	}

	if rows := batcher.Flush(); len(rows) > 0 {
		n, copyErr := opts.Copier.CopyRows(ctx, "public", table, columns, rows)
		if copyErr != nil {
			return total, copyErr
		}
		total += n
	}
	return total, nil
}

func rowCNPJ(row []any, columns []string) (string, bool) {
	for i, col := range columns {
		if col == "cnpj_basico" && i < len(row) {
			if s, ok := row[i].(string); ok {
				return s, true
			}
		}
	}
	return "", false
}

func sampleFilter(percent int) func(string) bool {
	return func(cnpj string) bool { return InSample(cnpj, percent) }
}
