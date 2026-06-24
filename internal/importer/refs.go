package importer

import (
	"context"
	"fmt"

	"busca-cnpj-2026/internal/loader"
	"busca-cnpj-2026/internal/parser"
)

func ImportReferences(ctx context.Context, ds Dataset, copier loader.BatchInserter) error {
	jobs := []struct {
		path    string
		table   string
		columns []string
		mapper  RowMapper
	}{
		{ds.CNAEs, "cnaes", []string{"codigo", "descricao", "secao", "divisao"}, cnaeRow},
		{ds.Motivos, "motivos", []string{"codigo", "descricao"}, refRow},
		{ds.Municipios, "municipios", []string{"codigo", "descricao"}, refRow},
		{ds.Naturezas, "naturezas", []string{"codigo", "descricao"}, refRow},
		{ds.Paises, "paises", []string{"codigo", "descricao"}, refRow},
		{ds.Qualificacoes, "qualificacoes", []string{"codigo", "descricao"}, refRow},
	}

	for _, job := range jobs {
		if job.path == "" {
			continue
		}
		if _, err := ImportFile(ctx, FileJob{
			Path: job.path, Table: job.table, Columns: job.columns, MapRow: job.mapper, Label: job.table,
		}, 1000, copier, nil, nil, nil, nil); err != nil {
			return fmt.Errorf("import %s: %w", job.table, err)
		}
	}
	return nil
}

func refRow(line []string, _ *parser.LookupStore) ([]any, error) {
	if len(line) < 2 {
		return nil, ErrReferenceRowColumns
	}
	return []any{cleanText(line[0]), cleanText(line[1])}, nil
}

func cnaeRow(line []string, _ *parser.LookupStore) ([]any, error) {
	if len(line) < 2 {
		return nil, ErrCNAERowColumns
	}
	code := cleanText(line[0])
	secao, divisao := "", ""
	if len(code) >= 1 {
		secao = code[:1]
	}
	if len(code) >= 2 {
		divisao = code[:2]
	}
	return []any{code, cleanText(line[1]), secao, divisao}, nil
}
