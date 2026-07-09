package importer

import (
	"fmt"
	"os"
	"path/filepath"
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

func discoverFiles(dir string) (Dataset, error) {
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
			ds.Simples = pickLatestPath(ds.Simples, path)
		case strings.Contains(name, "CNAECSV"):
			ds.CNAEs = pickLatestPath(ds.CNAEs, path)
		case strings.Contains(name, "MOTICSV"):
			ds.Motivos = pickLatestPath(ds.Motivos, path)
		case strings.Contains(name, "MUNICCSV"):
			ds.Municipios = pickLatestPath(ds.Municipios, path)
		case strings.Contains(name, "NATJUCSV"):
			ds.Naturezas = pickLatestPath(ds.Naturezas, path)
		case strings.Contains(name, "PAISCSV"):
			ds.Paises = pickLatestPath(ds.Paises, path)
		case strings.Contains(name, "QUALSCSV"):
			ds.Qualificacoes = pickLatestPath(ds.Qualificacoes, path)
		}
	}

	ds.Empresas = pickLatestByPartition(ds.Empresas)
	ds.Estabelecimentos = pickLatestByPartition(ds.Estabelecimentos)
	ds.Socios = pickLatestByPartition(ds.Socios)
	return ds, nil
}

func DiscoverDataset(dir string) (Dataset, error) {
	ds, err := discoverFiles(dir)
	if err != nil {
		return ds, err
	}
	if len(ds.Empresas) == 0 {
		return ds, fmt.Errorf("%w: %s", ErrNoEmpresasFiles, dir)
	}
	return ds, nil
}

// DiscoverReferences finds lookup CSV files without requiring fact-table dumps.
func DiscoverReferences(dir string) (Dataset, error) {
	ds, err := discoverFiles(dir)
	if err != nil {
		return ds, err
	}
	if ds.CNAEs == "" && ds.Motivos == "" && ds.Municipios == "" &&
		ds.Naturezas == "" && ds.Paises == "" && ds.Qualificacoes == "" {
		return ds, fmt.Errorf("no reference CSV files in %s", dir)
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
