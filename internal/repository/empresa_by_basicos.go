package repository

import (
	"context"
	"fmt"

	"github.com/lib/pq"

	"busca-cnpj-2026/internal/models"
)

func (r *EmpresaRepository) ListEmpresasByCNPJBasicos(
	ctx context.Context,
	basicos []string,
) ([]models.Empresa, error) {
	if len(basicos) == 0 {
		return nil, nil
	}
	query := `
		SELECT uuid_id, cnpj_basico, razao_social, natureza_juridica,
			qualificacao_responsavel, capital_social, porte_empresa,
			ente_federativo_responsavel, created_at, updated_at
		FROM empresas
		WHERE cnpj_basico = ANY($1)`
	rows, err := r.db.QueryContext(ctx, query, pq.Array(basicos))
	if err != nil {
		return nil, fmt.Errorf("list empresas by basicos: %w", err)
	}
	defer rows.Close()
	byBasico := make(map[string]models.Empresa, len(basicos))
	for rows.Next() {
		var emp models.Empresa
		if err := rows.Scan(
			&emp.UUIDID, &emp.CNPJBasico, &emp.RazaoSocial, &emp.NaturezaJuridica,
			&emp.QualificacaoResponsavel, &emp.CapitalSocial, &emp.PorteEmpresa,
			&emp.EnteFederativoResponsavel, &emp.CreatedAt, &emp.UpdatedAt,
		); err != nil {
			return nil, err
		}
		byBasico[emp.CNPJBasico] = emp
	}
	if err := rows.Err(); err != nil {
		return nil, err
	}
	out := make([]models.Empresa, 0, len(basicos))
	for _, b := range basicos {
		if emp, ok := byBasico[b]; ok {
			out = append(out, emp)
		}
	}
	return out, nil
}
