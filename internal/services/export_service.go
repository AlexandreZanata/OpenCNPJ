package services

import (
	"context"
	"fmt"
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

// ExportCSV uses PostgreSQL COPY TO STDOUT for maximum performance (10-50x faster)
// Streams data directly from PostgreSQL to HTTP response without intermediate buffers
func (s *ExportService) ExportCSV(ctx context.Context, w io.Writer, req models.ExportRequest) error {
	// Set default columns if not provided
	if len(req.SelectedColumns) == 0 {
		req.SelectedColumns = []string{"cnpj_completo", "nome_fantasia", "razao_social", "cnae_fiscal_principal", "uf", "municipio"}
	}

	// Determine which repository to use based on filters
	if req.Filters.CNPJCompleto != "" || req.Filters.NomeFantasia != "" || req.Filters.CNAEPrincipal != "" {
		// Export estabelecimentos using COPY TO STDOUT
		return s.exportEstabelecimentosCSV(ctx, w, req)
	} else {
		// Export empresas using COPY TO STDOUT
		return s.exportEmpresasCSV(ctx, w, req)
	}
}

// exportEstabelecimentosCSV uses COPY TO STDOUT for ultra-fast streaming export
func (s *ExportService) exportEstabelecimentosCSV(ctx context.Context, w io.Writer, req models.ExportRequest) error {
	// Use COPY TO STDOUT for maximum performance
	// This streams directly from PostgreSQL without loading data into memory
	return s.estabelecimentoRepo.ExportToCSV(ctx, w, req.Filters, req.SelectedColumns)
}

// exportEmpresasCSV uses COPY TO STDOUT for ultra-fast streaming export
func (s *ExportService) exportEmpresasCSV(ctx context.Context, w io.Writer, req models.ExportRequest) error {
	// Use COPY TO STDOUT for maximum performance
	// This streams directly from PostgreSQL without loading data into memory
	return s.empresaRepo.ExportToCSV(ctx, w, req.Filters, req.SelectedColumns)
}

func (s *ExportService) buildEstabelecimentoRow(est models.EstabelecimentoCompleto, columns []string) []string {
	row := make([]string, len(columns))
	for i, col := range columns {
		switch col {
		case "cnpj_completo":
			row[i] = est.CNPJCompleto
		case "cnpj_basico":
			row[i] = est.CNPJBasico
		case "nome_fantasia":
			if est.NomeFantasia.Valid {
				row[i] = est.NomeFantasia.String
			}
		case "razao_social":
			if est.RazaoSocial.Valid {
				row[i] = est.RazaoSocial.String
			}
		case "cnae_fiscal_principal":
			if est.CNAEFiscalPrincipal.Valid {
				row[i] = est.CNAEFiscalPrincipal.String
			}
		case "cnae_descricao":
			if est.CNAEDescricao.Valid {
				row[i] = est.CNAEDescricao.String
			}
		case "uf":
			if est.UF.Valid {
				row[i] = est.UF.String
			}
		case "municipio":
			if est.Municipio.Valid {
				row[i] = est.Municipio.String
			}
		case "municipio_nome":
			if est.MunicipioNome.Valid {
				row[i] = est.MunicipioNome.String
			}
		case "situacao_cadastral":
			if est.SituacaoCadastral.Valid {
				row[i] = est.SituacaoCadastral.String
			}
		case "logradouro":
			if est.Logradouro.Valid {
				row[i] = est.Logradouro.String
			}
		case "numero":
			if est.Numero.Valid {
				row[i] = est.Numero.String
			}
		case "bairro":
			if est.Bairro.Valid {
				row[i] = est.Bairro.String
			}
		case "cep":
			if est.CEP.Valid {
				row[i] = est.CEP.String
			}
		default:
			row[i] = ""
		}
	}
	return row
}

func (s *ExportService) buildEmpresaRow(emp models.Empresa, columns []string) []string {
	row := make([]string, len(columns))
	for i, col := range columns {
		switch col {
		case "cnpj_basico":
			row[i] = emp.CNPJBasico
		case "razao_social":
			row[i] = emp.RazaoSocial
		case "natureza_juridica":
			if emp.NaturezaJuridica.Valid {
				row[i] = emp.NaturezaJuridica.String
			}
		case "capital_social":
			if emp.CapitalSocial.Valid {
				row[i] = fmt.Sprintf("%.2f", emp.CapitalSocial.Float64)
			}
		case "porte_empresa":
			if emp.PorteEmpresa.Valid {
				row[i] = emp.PorteEmpresa.String
			}
		default:
			row[i] = ""
		}
	}
	return row
}
