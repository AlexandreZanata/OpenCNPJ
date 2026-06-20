package services

import (
	"context"
	"io"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

type ExportService struct {
	estabelecimentoRepo *repository.EstabelecimentoRepository
	empresaRepo         *repository.EmpresaRepository
}

func NewExportService() *ExportService {
	return &ExportService{
		estabelecimentoRepo: repository.NewEstabelecimentoRepository(),
		empresaRepo:         repository.NewEmpresaRepository(),
	}
}

// Streams data directly from PostgreSQL to HTTP response without intermediate buffers.
//
//nolint:gocritic // Keeping value argument to avoid broad API churn now.
func (s *ExportService) ExportCSV(ctx context.Context, w io.Writer, req models.ExportRequest) error {
	if len(req.SelectedColumns) == 0 {
		req.SelectedColumns = []string{
			"cnpj_completo",
			"nome_fantasia",
			"razao_social",
			"cnae_fiscal_principal",
			"uf",
			"municipio",
		}
	}

	if exportUsesEstabelecimentos(req.Filters, req.SelectedColumns) {
		return s.exportEstabelecimentosCSV(ctx, w, req)
	}
	return s.exportEmpresasCSV(ctx, w, req)
}

func exportUsesEstabelecimentos(filters models.SearchFilters, columns []string) bool {
	if filters.CNPJCompleto != "" || filters.NomeFantasia != "" || filters.CNAEPrincipal != "" ||
		filters.UF != "" || filters.Municipio != "" || filters.SituacaoCadastral != "" || filters.CEP != "" {
		return true
	}

	estabColumns := map[string]struct{}{
		"cnpj_completo": {}, "nome_fantasia": {}, "cnae_fiscal_principal": {},
		"cnae_descricao": {}, "uf": {}, "municipio": {}, "municipio_nome": {},
		"situacao_cadastral": {}, "logradouro": {}, "numero": {}, "bairro": {}, "cep": {},
	}
	for _, col := range columns {
		if _, ok := estabColumns[col]; ok {
			return true
		}
	}
	return false
}

// exportEstabelecimentosCSV uses COPY TO STDOUT for ultra-fast streaming export.
//
//nolint:gocritic // Keeping value argument to avoid broad API churn now.
func (s *ExportService) exportEstabelecimentosCSV(ctx context.Context, w io.Writer, req models.ExportRequest) error {
	// Use COPY TO STDOUT for maximum performance
	// This streams directly from PostgreSQL without loading data into memory
	return s.estabelecimentoRepo.ExportToCSV(ctx, w, req.Filters, req.SelectedColumns)
}

// exportEmpresasCSV uses COPY TO STDOUT for ultra-fast streaming export.
//
//nolint:gocritic // Keeping value argument to avoid broad API churn now.
func (s *ExportService) exportEmpresasCSV(ctx context.Context, w io.Writer, req models.ExportRequest) error {
	// Use COPY TO STDOUT for maximum performance
	// This streams directly from PostgreSQL without loading data into memory
	return s.empresaRepo.ExportToCSV(ctx, w, req.Filters, req.SelectedColumns)
}
