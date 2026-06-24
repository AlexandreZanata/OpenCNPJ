package importer

import (
	"bufio"
	"context"
	"fmt"
	"io"
	"os"
	"sync/atomic"
	"time"

	"busca-cnpj-2026/internal/loader"
	"busca-cnpj-2026/internal/metrics"
	"busca-cnpj-2026/internal/parser"
)

const readerBuffer = 4 * 1024 * 1024

type RowMapper func([]string, *parser.LookupStore) ([]any, error)

type Timings struct {
	ParseNs atomic.Int64
	CopyNs  atomic.Int64
}

type FileJob struct {
	Path    string
	Table   string
	Columns []string
	Limit   int64
	MapRow  RowMapper
	Label   string
}

func ImportFile(
	ctx context.Context,
	job FileJob,
	batchSize int,
	copier loader.BatchInserter,
	lookups *parser.LookupStore,
	collector *metrics.Collector,
	timings *Timings,
	dedupe *DedupeSet,
) (int64, error) {
	// #nosec G304 -- path comes from trusted dataset manifest.
	file, err := os.Open(job.Path)
	if err != nil {
		return 0, fmt.Errorf("open %s: %w", job.Path, err)
	}
	defer file.Close()

	reader := parser.NewCSVReader(bufio.NewReaderSize(file, readerBuffer))
	batcher := loader.NewBatcher(batchSize)
	var imported int64

	for {
		if err := ctx.Err(); err != nil {
			return imported, err
		}
		if job.Limit > 0 && imported >= job.Limit {
			break
		}

		line, err := reader.Read()
		if err != nil {
			if err == io.EOF {
				break
			}
			return imported, fmt.Errorf("read %s: %w", job.Path, err)
		}
		if collector != nil {
			collector.AddBytes(metrics.CSVRecordBytes(line))
		}

		row, ok, err := mapImportRow(job, line, lookups, collector, timings)
		if err != nil {
			return imported, err
		}
		if !ok {
			continue
		}
		if skipDedupe(job.Table, row, dedupe) {
			continue
		}

		rows, flush := batcher.Add(row)
		if !flush {
			continue
		}
		n, copyErr := copyImportBatch(ctx, job, copier, rows, timings, collector)
		if copyErr != nil {
			return imported, copyErr
		}
		imported += n
	}

	if tail := batcher.Flush(); len(tail) > 0 {
		n, copyErr := copyImportBatch(ctx, job, copier, tail, timings, collector)
		if copyErr != nil {
			return imported, fmt.Errorf("copy %s flush: %w", job.Table, copyErr)
		}
		imported += n
	}

	return imported, nil
}

func mapImportRow(
	job FileJob,
	line []string,
	lookups *parser.LookupStore,
	collector *metrics.Collector,
	timings *Timings,
) ([]any, bool, error) {
	parseStart := time.Now()
	row, mapErr := job.MapRow(line, lookups)
	if timings != nil {
		timings.ParseNs.Add(time.Since(parseStart).Nanoseconds())
	}
	if mapErr != nil {
		if collector != nil {
			collector.AddError()
		}
		return nil, false, nil
	}
	return row, true, nil
}

func skipDedupe(table string, row []any, dedupe *DedupeSet) bool {
	if dedupe == nil {
		return false
	}
	key, use := dedupeKey(table, row)
	return use && dedupe.Seen(key)
}

func copyImportBatch(
	ctx context.Context,
	job FileJob,
	copier loader.BatchInserter,
	rows [][]any,
	timings *Timings,
	collector *metrics.Collector,
) (int64, error) {
	copyStart := time.Now()
	if _, copyErr := copier.CopyRows(ctx, "public", job.Table, job.Columns, rows); copyErr != nil {
		return 0, fmt.Errorf("copy %s: %w", job.Table, copyErr)
	}
	if timings != nil {
		timings.CopyNs.Add(time.Since(copyStart).Nanoseconds())
	}
	n := int64(len(rows))
	if collector != nil {
		collector.AddRows(n)
	}
	return n, nil
}
