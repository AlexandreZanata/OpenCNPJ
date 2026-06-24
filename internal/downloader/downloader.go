package downloader

import (
	"archive/zip"
	"context"
	"fmt"
	"io"
	"log"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"
)

var cnpjFileMarkers = []string{
	"CNAECSV", "MOTICSV", "MUNICCSV", "NATJUCSV", "PAISCSV", "QUALSCSV",
	"EMPRECSV", "ESTABELE", "SOCIOCSV", "SIMPLES",
}

var referenceArchives = map[string]struct{}{
	"Cnaes.zip":         {},
	"Motivos.zip":       {},
	"Municipios.zip":    {},
	"Naturezas.zip":     {},
	"Paises.zip":        {},
	"Qualificacoes.zip": {},
}

// Options controls the download bot behavior.
type Options struct {
	OutputDir     string
	Month         string
	Workers       int
	KeepZIP       bool
	RetryAttempts int
	RetryDelay    time.Duration
}

// Result summarizes a completed download run.
type Result struct {
	Month         string
	FilesTotal    int
	FilesSkipped  int
	FilesDownload int
	CSVExtracted  int
}

// Downloader orchestrates listing, downloading and extracting CNPJ open data.
type Downloader struct {
	client *Client
	opts   Options
}

func NewDownloader(client *Client, opts Options) *Downloader {
	if opts.OutputDir == "" {
		opts.OutputDir = "./data"
	}
	if opts.Workers <= 0 {
		opts.Workers = 4
	}
	if opts.RetryAttempts <= 0 {
		opts.RetryAttempts = 3
	}
	if opts.RetryDelay <= 0 {
		opts.RetryDelay = 5 * time.Second
	}
	return &Downloader{client: client, opts: opts}
}

// ResolveMonth picks the target folder: explicit month, current month if published, or latest.
func ResolveMonth(available []string, requested string, now time.Time) (string, bool, error) {
	if len(available) == 0 {
		return "", false, fmt.Errorf("empty month list")
	}

	if requested != "" {
		for _, m := range available {
			if m == requested {
				return m, false, nil
			}
		}
		return "", false, fmt.Errorf("month %s not available (latest: %s)", requested, available[len(available)-1])
	}

	current := now.Format("2006-01")
	for _, m := range available {
		if m == current {
			return m, false, nil
		}
	}

	latest := available[len(available)-1]
	return latest, true, nil
}

func (d *Downloader) Run(ctx context.Context) (*Result, error) {
	if err := os.MkdirAll(d.opts.OutputDir, 0o755); err != nil {
		return nil, fmt.Errorf("create output directory: %w", err)
	}

	months, err := d.client.ListMonthDirectories(ctx)
	if err != nil {
		return nil, err
	}

	month, usedFallback, err := ResolveMonth(months, d.opts.Month, time.Now())
	if err != nil {
		return nil, err
	}
	if usedFallback {
		log.Printf("warning: %s data not yet published; using %s", time.Now().Format("2006-01"), month)
	}
	log.Printf("downloading %s data to %s", month, d.opts.OutputDir)

	files, err := d.client.ListZipFiles(ctx, month)
	if err != nil {
		return nil, err
	}

	reference, data := splitFiles(files)
	ordered := append(reference, data...)

	result := &Result{Month: month, FilesTotal: len(ordered)}
	for _, filename := range ordered {
		csvCount, skipped, err := d.processFile(ctx, month, filename)
		if err != nil {
			return result, fmt.Errorf("%s: %w", filename, err)
		}
		if skipped {
			result.FilesSkipped++
			continue
		}
		result.FilesDownload++
		result.CSVExtracted += csvCount
	}

	log.Printf(
		"done: %d files downloaded, %d skipped (already present), %d CSVs extracted",
		result.FilesDownload, result.FilesSkipped, result.CSVExtracted,
	)
	return result, nil
}

func splitFiles(files []string) (reference, data []string) {
	for _, f := range files {
		if _, ok := referenceArchives[f]; ok {
			reference = append(reference, f)
		} else {
			data = append(data, f)
		}
	}
	sort.Strings(reference)
	sort.Strings(data)
	return reference, data
}

func (d *Downloader) processFile(ctx context.Context, month, filename string) (csvCount int, skipped bool, err error) {
	zipPath := filepath.Join(d.opts.OutputDir, filename)
	if d.isDone(month, filename) {
		log.Printf("  [skip] %s (already downloaded)", filename)
		return 0, true, nil
	}

	log.Printf("  [download] %s", filename)
	if err := d.downloadWithRetry(ctx, month, filename, zipPath); err != nil {
		return 0, false, err
	}

	count, err := extractCNPJCSVs(zipPath, d.opts.OutputDir)
	if err != nil {
		return 0, false, err
	}

	if !d.opts.KeepZIP {
		if removeErr := os.Remove(zipPath); removeErr != nil {
			log.Printf("  warning: could not remove %s: %v", zipPath, removeErr)
		}
	}

	if err := d.markDone(month, filename); err != nil {
		log.Printf("  warning: could not write marker for %s: %v", filename, err)
	}

	return count, false, nil
}

func (d *Downloader) markerPath(month, filename string) string {
	return filepath.Join(d.opts.OutputDir, ".downloaded", month, filename+".done")
}

func (d *Downloader) isDone(month, filename string) bool {
	_, err := os.Stat(d.markerPath(month, filename))
	return err == nil
}

func (d *Downloader) markDone(month, filename string) error {
	path := d.markerPath(month, filename)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(month), 0o644)
}

func (d *Downloader) downloadWithRetry(ctx context.Context, month, filename, dest string) error {
	var lastErr error
	for attempt := 1; attempt <= d.opts.RetryAttempts; attempt++ {
		lastErr = d.downloadFile(ctx, month, filename, dest)
		if lastErr == nil {
			return nil
		}
		if attempt < d.opts.RetryAttempts {
			log.Printf("  attempt %d/%d failed: %v; waiting %s", attempt, d.opts.RetryAttempts, lastErr, d.opts.RetryDelay)
			select {
			case <-ctx.Done():
				return ctx.Err()
			case <-time.After(d.opts.RetryDelay):
			}
		}
	}
	return lastErr
}

func (d *Downloader) downloadFile(ctx context.Context, month, filename, dest string) error {
	body, _, err := d.client.Download(ctx, month, filename)
	if err != nil {
		return err
	}
	defer body.Close()

	tmp := dest + ".part"
	f, err := os.Create(tmp)
	if err != nil {
		return err
	}

	written, copyErr := io.Copy(f, body)
	closeErr := f.Close()
	if copyErr != nil {
		_ = os.Remove(tmp)
		return copyErr
	}
	if closeErr != nil {
		_ = os.Remove(tmp)
		return closeErr
	}

	if err := os.Rename(tmp, dest); err != nil {
		_ = os.Remove(tmp)
		return err
	}

	log.Printf("  [ok] %s (%d bytes)", filename, written)
	return nil
}

func extractCNPJCSVs(zipPath, outputDir string) (int, error) {
	r, err := zip.OpenReader(zipPath)
	if err != nil {
		return 0, fmt.Errorf("open zip: %w", err)
	}
	defer r.Close()

	count := 0
	for _, f := range r.File {
		if !isCNPJMember(f.Name) {
			continue
		}
		dest := filepath.Join(outputDir, filepath.Base(f.Name))
		if err := extractZipMember(f, dest); err != nil {
			return count, err
		}
		count++
	}
	return count, nil
}

func isCNPJMember(name string) bool {
	upper := strings.ToUpper(name)
	for _, marker := range cnpjFileMarkers {
		if strings.Contains(upper, marker) {
			return true
		}
	}
	return false
}

func extractZipMember(f *zip.File, dest string) error {
	rc, err := f.Open()
	if err != nil {
		return err
	}
	defer rc.Close()

	if err := os.MkdirAll(filepath.Dir(dest), 0o755); err != nil {
		return err
	}

	tmp := dest + ".part"
	out, err := os.Create(tmp)
	if err != nil {
		return err
	}

	if _, err := io.Copy(out, rc); err != nil {
		_ = out.Close()
		_ = os.Remove(tmp)
		return err
	}
	if err := out.Close(); err != nil {
		_ = os.Remove(tmp)
		return err
	}
	return os.Rename(tmp, dest)
}
