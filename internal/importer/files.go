package importer

import (
	"os"
	"path/filepath"
	"strings"
)

// DataFiles groups discovered Receita Federal CSV paths by entity.
type DataFiles struct {
	CNAEs         string
	Motivos       string
	Municipios    string
	Naturezas     string
	Paises        string
	Qualificacoes string
	Empresas      []string
	Estabelec     []string
	Socios        []string
	Simples       string
}

func DiscoverDataFiles(dir string) (DataFiles, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return DataFiles{}, err
	}

	var out DataFiles
	for _, e := range entries {
		if e.IsDir() {
			continue
		}
		name := strings.ToUpper(e.Name())
		path := filepath.Join(dir, e.Name())
		switch {
		case strings.Contains(name, "CNAECSV"):
			out.CNAEs = pickLatestPath(out.CNAEs, path)
		case strings.Contains(name, "MOTICSV"):
			out.Motivos = pickLatestPath(out.Motivos, path)
		case strings.Contains(name, "MUNICCSV"):
			out.Municipios = pickLatestPath(out.Municipios, path)
		case strings.Contains(name, "NATJUCSV"):
			out.Naturezas = pickLatestPath(out.Naturezas, path)
		case strings.Contains(name, "PAISCSV"):
			out.Paises = pickLatestPath(out.Paises, path)
		case strings.Contains(name, "QUALSCSV"):
			out.Qualificacoes = pickLatestPath(out.Qualificacoes, path)
		case strings.Contains(name, "EMPRECSV"):
			out.Empresas = append(out.Empresas, path)
		case strings.Contains(name, "ESTABELE"):
			out.Estabelec = append(out.Estabelec, path)
		case strings.Contains(name, "SOCIOCSV"):
			out.Socios = append(out.Socios, path)
		case strings.Contains(name, "SIMPLES"):
			out.Simples = pickLatestPath(out.Simples, path)
		}
	}

	out.Empresas = pickLatestByPartition(out.Empresas)
	out.Estabelec = pickLatestByPartition(out.Estabelec)
	out.Socios = pickLatestByPartition(out.Socios)
	return out, nil
}
