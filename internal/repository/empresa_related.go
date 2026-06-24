package repository

import (
	"context"
	"fmt"

	"github.com/lib/pq"

	"busca-cnpj-2026/internal/models"
)

func (r *EmpresaRepository) ListEmpresasFullByBasicos(
	ctx context.Context,
	basicos []string,
) (map[string]models.EmpresaFull, error) {
	if len(basicos) == 0 {
		return map[string]models.EmpresaFull{}, nil
	}
	rows, err := r.db.QueryContext(ctx, `
		SELECT emp.uuid_id, emp.cnpj_basico, emp.razao_social, emp.natureza_juridica,
			emp.qualificacao_responsavel, emp.capital_social, emp.porte_empresa,
			emp.ente_federativo_responsavel, emp.created_at, emp.updated_at,
			n.descricao, q.descricao
		FROM empresas emp
		LEFT JOIN naturezas n ON emp.natureza_juridica = n.codigo
		LEFT JOIN qualificacoes q ON emp.qualificacao_responsavel = q.codigo
		WHERE emp.cnpj_basico = ANY($1)
	`, pq.Array(basicos))
	if err != nil {
		return nil, fmt.Errorf("list empresas full: %w", err)
	}
	defer rows.Close()

	out := make(map[string]models.EmpresaFull, len(basicos))
	for rows.Next() {
		var full models.EmpresaFull
		if err := rows.Scan(
			&full.UUIDID, &full.CNPJBasico, &full.RazaoSocial, &full.NaturezaJuridica,
			&full.QualificacaoResponsavel, &full.CapitalSocial, &full.PorteEmpresa,
			&full.EnteFederativoResponsavel, &full.CreatedAt, &full.UpdatedAt,
			&full.NaturezaDescricao, &full.QualificacaoDescricao,
		); err != nil {
			return nil, fmt.Errorf("scan empresa full: %w", err)
		}
		out[full.CNPJBasico] = full
	}
	return out, rows.Err()
}

func (r *EstabelecimentoRepository) ListByCNPJBasicos(
	ctx context.Context,
	basicos []string,
) ([]models.EstabelecimentoCompleto, error) {
	if len(basicos) == 0 {
		return []models.EstabelecimentoCompleto{}, nil
	}
	query := `SELECT ` + estabelecimentoCompletoSelect + estabelecimentoCompletoFrom + `
		WHERE e.cnpj_basico = ANY($1)
		ORDER BY e.cnpj_basico, e.cnpj_ordem`
	rows, err := r.db.QueryContext(ctx, query, pq.Array(basicos))
	if err != nil {
		return nil, fmt.Errorf("list estabelecimentos by basicos: %w", err)
	}
	defer rows.Close()
	return scanEstabelecimentoRows(rows)
}

func (r *EmpresaRepository) ListSociosByCNPJBasicos(
	ctx context.Context,
	basicos []string,
) ([]models.Socio, error) {
	if len(basicos) == 0 {
		return []models.Socio{}, nil
	}
	rows, err := r.db.QueryContext(ctx, `
		SELECT id, uuid_id, cnpj_basico, identificador_socio, nome_socio, cpf_cnpj_socio,
			qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
			nome_representante, qualificacao_representante, faixa_etaria, created_at
		FROM socios
		WHERE cnpj_basico = ANY($1)
		ORDER BY cnpj_basico, nome_socio
	`, pq.Array(basicos))
	if err != nil {
		return nil, fmt.Errorf("list socios: %w", err)
	}
	defer rows.Close()

	out := make([]models.Socio, 0)
	for rows.Next() {
		var s models.Socio
		if err := rows.Scan(
			&s.ID, &s.UUIDID, &s.CNPJBasico, &s.IdentificadorSocio, &s.NomeSocio,
			&s.CPFCNPJSocio, &s.QualificacaoSocio, &s.DataEntradaSociedade, &s.Pais,
			&s.RepresentanteLegal, &s.NomeRepresentante, &s.QualificacaoRepresentante,
			&s.FaixaEtaria, &s.CreatedAt,
		); err != nil {
			return nil, fmt.Errorf("scan socio: %w", err)
		}
		out = append(out, s)
	}
	return out, rows.Err()
}

func (r *EmpresaRepository) ListSimplesByCNPJBasicos(
	ctx context.Context,
	basicos []string,
) (map[string]models.Simples, error) {
	if len(basicos) == 0 {
		return map[string]models.Simples{}, nil
	}
	rows, err := r.db.QueryContext(ctx, `
		SELECT uuid_id, cnpj_basico, opcao_simples, data_opcao_simples, data_exclusao_simples,
			opcao_mei, data_opcao_mei, data_exclusao_mei
		FROM simples
		WHERE cnpj_basico = ANY($1)
	`, pq.Array(basicos))
	if err != nil {
		return nil, fmt.Errorf("list simples: %w", err)
	}
	defer rows.Close()

	out := make(map[string]models.Simples, len(basicos))
	for rows.Next() {
		var s models.Simples
		if err := rows.Scan(
			&s.UUIDID, &s.CNPJBasico, &s.OpcaoSimples, &s.DataOpcaoSimples,
			&s.DataExclusaoSimples, &s.OpcaoMEI, &s.DataOpcaoMEI, &s.DataExclusaoMEI,
		); err != nil {
			return nil, fmt.Errorf("scan simples: %w", err)
		}
		out[s.CNPJBasico] = s
	}
	return out, rows.Err()
}
