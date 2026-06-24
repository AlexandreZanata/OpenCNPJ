package importer

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

const sampleProbeSize = 1 << 20

// Dataset groups Receita Federal CSV files under a data directory.
type Dataset struct {
	Empresas         []string
	Estabelecimentos []string
	Socios           []string
	Simples          string
	CNAEs            string
	Motivos          string
	Municipios       string
	Naturezas        string
	Paises           string
	Qualificacoes    string
}

func DiscoverDataset(dir string) (Dataset, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return Dataset{}, fmt.Errorf("read data dir: %w", err)
	}

	var ds Dataset
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}
		name := entry.Name()
		path := filepath.Join(dir, name)
		switch {
		case strings.Contains(name, "EMPRECSV"):
			ds.Empresas = append(ds.Empresas, path)
		case strings.Contains(name, "ESTABELE"):
			ds.Estabelecimentos = append(ds.Estabelecimentos, path)
		case strings.Contains(name, "SOCIOCSV"):
			ds.Socios = append(ds.Socios, path)
		case strings.Contains(name, "SIMPLES"):
			ds.Simples = path
		case strings.Contains(name, "CNAECSV"):
			ds.CNAEs = path
		case strings.Contains(name, "MOTICSV"):
			ds.Motivos = path
		case strings.Contains(name, "MUNICCSV"):
			ds.Municipios = path
		case strings.Contains(name, "NATJUCSV"):
			ds.Naturezas = path
		case strings.Contains(name, "PAISCSV"):
			ds.Paises = path
		case strings.Contains(name, "QUALSCSV"):
			ds.Qualificacoes = path
		}
	}

	sort.Strings(ds.Empresas)
	sort.Strings(ds.Estabelecimentos)
	sort.Strings(ds.Socios)

	if len(ds.Empresas) == 0 {
		return ds, fmt.Errorf("%w: %s", ErrNoEmpresasFiles, dir)
	}
	return ds, nil
}

func RowLimit(path string, samplePercent int) (int64, error) {
	if samplePercent >= 100 {
		return 0, nil
	}
	if samplePercent <= 0 {
		return 0, ErrInvalidSamplePercent
	}

	estimated, err := estimateLines(path)
	if err != nil {
		return 0, err
	}
	limit := estimated * int64(samplePercent) / 100
	if limit < 1 {
		limit = 1
	}
	return limit, nil
}

func estimateLines(path string) (int64, error) {
	info, err := os.Stat(path)
	if err != nil {
		return 0, err
	}
	if info.Size() == 0 {
		return 0, nil
	}

	probe := sampleProbeSize
	if info.Size() < int64(probe) {
		probe = int(info.Size())
	}

	// #nosec G304 -- path comes from trusted dataset manifest.
	file, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer file.Close()

	buf := make([]byte, probe)
	n, err := file.Read(buf)
	if err != nil && n == 0 {
		return 0, err
	}

	newlines := int64(strings.Count(string(buf[:n]), "\n"))
	if newlines == 0 {
		newlines = 1
	}
	ratio := float64(info.Size()) / float64(n)
	return int64(float64(newlines) * ratio), nil
}

func countLinesQuick(path string) (int64, error) {
	// #nosec G304 -- path comes from trusted dataset manifest.
	file, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	defer file.Close()

	scanner := bufio.NewScanner(file)
	scanner.Buffer(make([]byte, 1024*1024), 1024*1024)
	var lines int64
	for scanner.Scan() {
		lines++
	}
	return lines, scanner.Err()
}
