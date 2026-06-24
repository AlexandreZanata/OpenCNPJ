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

		parseStart := time.Now()
		row, mapErr := job.MapRow(line, lookups)
		if timings != nil {
			timings.ParseNs.Add(time.Since(parseStart).Nanoseconds())
		}
		if mapErr != nil {
			if collector != nil {
				collector.AddError()
			}
			continue
		}
		if dedupe != nil {
			if key, use := dedupeKey(job.Table, row); use && dedupe.Seen(key) {
				continue
			}
		}

		rows, flush := batcher.Add(row)
		if flush {
			copyStart := time.Now()
			if _, copyErr := copier.CopyRows(ctx, "public", job.Table, job.Columns, rows); copyErr != nil {
				return imported, fmt.Errorf("copy %s: %w", job.Table, copyErr)
			}
			if timings != nil {
				timings.CopyNs.Add(time.Since(copyStart).Nanoseconds())
			}
			if collector != nil {
				collector.AddRows(int64(len(rows)))
			}
			imported += int64(len(rows))
		}
	}

	if tail := batcher.Flush(); len(tail) > 0 {
		copyStart := time.Now()
		if _, copyErr := copier.CopyRows(ctx, "public", job.Table, job.Columns, tail); copyErr != nil {
			return imported, fmt.Errorf("copy %s flush: %w", job.Table, copyErr)
		}
		if timings != nil {
			timings.CopyNs.Add(time.Since(copyStart).Nanoseconds())
		}
		if collector != nil {
			collector.AddRows(int64(len(tail)))
		}
		imported += int64(len(tail))
	}

	return imported, nil
}
