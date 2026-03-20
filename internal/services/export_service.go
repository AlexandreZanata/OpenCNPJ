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
	// Set default columns if not provided
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

	// Determine which repository to use based on filters
	if req.Filters.CNPJCompleto != "" || req.Filters.NomeFantasia != "" || req.Filters.CNAEPrincipal != "" {
		// Export estabelecimentos using COPY TO STDOUT
		return s.exportEstabelecimentosCSV(ctx, w, req)
	}
	// Export empresas using COPY TO STDOUT
	return s.exportEmpresasCSV(ctx, w, req)
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
