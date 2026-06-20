package repository

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"strings"

	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/models"
)

type EstabelecimentoRepository struct {
	db *sql.DB
}

func NewEstabelecimentoRepository() *EstabelecimentoRepository {
	return &EstabelecimentoRepository{
		db: database.DB,
	}
}

//nolint:gocritic // Keeping value argument to avoid broad API churn now.
func (r *EstabelecimentoRepository) SearchEstabelecimentos(
	ctx context.Context,
	filters models.SearchFilters,
) ([]models.EstabelecimentoCompleto, int64, error) {
	query := `
		SELECT 
			e.id, e.uuid_id, e.cnpj_basico, e.cnpj_ordem, e.cnpj_dv, e.cnpj_completo,
			e.identificador_matriz_filial, e.nome_fantasia, e.situacao_cadastral,
			e.data_situacao_cadastral, e.motivo_situacao_cadastral,
			e.nome_cidade_exterior, e.pais, e.data_inicio_atividade,
			e.cnae_fiscal_principal, e.cnae_fiscal_secundaria,
			e.tipo_logradouro, e.logradouro, e.numero, e.complemento,
			e.bairro, e.cep, e.uf, e.municipio,
			e.ddd_1, e.telefone_1, e.ddd_2, e.telefone_2,
			e.ddd_fax, e.fax, e.email, e.situacao_especial,
			e.data_situacao_especial, e.created_at,
			emp.razao_social,
			c.descricao as cnae_descricao,
			m.descricao as municipio_nome
		FROM estabelecimentos e
		INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
		LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
		LEFT JOIN municipios m ON e.municipio = m.codigo
		WHERE 1=1
	`

	args := []interface{}{}
	argPos := 1

	if filters.CNPJCompleto != "" {
		query += fmt.Sprintf(" AND e.cnpj_completo = $%d", argPos)
		args = append(args, filters.CNPJCompleto)
		argPos++
	}

	if filters.UUIDID != "" {
		query += fmt.Sprintf(" AND e.uuid_id = $%d", argPos)
		args = append(args, filters.UUIDID)
		argPos++
	}

	if filters.CNPJBasico != "" {
		query += fmt.Sprintf(" AND e.cnpj_basico = $%d", argPos)
		args = append(args, filters.CNPJBasico)
		argPos++
	}

	if filters.NomeFantasia != "" {
		query += fmt.Sprintf(" AND e.nome_fantasia ILIKE $%d", argPos)
		args = append(args, "%"+filters.NomeFantasia+"%")
		argPos++
	}

	if filters.CNAEPrincipal != "" {
		query += fmt.Sprintf(" AND e.cnae_fiscal_principal = $%d", argPos)
		args = append(args, filters.CNAEPrincipal)
		argPos++
	}

	if filters.UF != "" {
		query += fmt.Sprintf(" AND e.uf = $%d", argPos)
		args = append(args, filters.UF)
		argPos++
	}

	if filters.Municipio != "" {
		query += fmt.Sprintf(" AND e.municipio = $%d", argPos)
		args = append(args, filters.Municipio)
		argPos++
	}

	if filters.SituacaoCadastral != "" {
		query += fmt.Sprintf(" AND e.situacao_cadastral = $%d", argPos)
		args = append(args, filters.SituacaoCadastral)
		argPos++
	}

	if filters.CEP != "" {
		query += fmt.Sprintf(" AND e.cep = $%d", argPos)
		args = append(args, filters.CEP)
		argPos++
	}

	// Count query — skip for fuzzy text searches (ILIKE on 70M+ rows).
	skipCount := hasFuzzyTextFilter(filters)
	if filters.Limit <= 0 {
		filters.Limit = 100
	}
	queryLimit := filters.Limit
	if skipCount {
		queryLimit = filters.Limit + 1
	}

	var total int64
	if !skipCount {
		countQuery := "SELECT COUNT(*) FROM (" + query + ") AS count_query"
		if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
			return nil, 0, fmt.Errorf("failed to count estabelecimentos: %w", err)
		}
	}

	// Add pagination
	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	query += fmt.Sprintf(" ORDER BY e.nome_fantasia LIMIT $%d OFFSET $%d", argPos, argPos+1)
	args = append(args, queryLimit, filters.Offset)

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, 0, fmt.Errorf("failed to query estabelecimentos: %w", err)
	}
	defer rows.Close()

	var estabelecimentos []models.EstabelecimentoCompleto
	for rows.Next() {
		var est models.EstabelecimentoCompleto
		err := rows.Scan(
			&est.ID,
			&est.UUIDID,
			&est.CNPJBasico,
			&est.CNPJOrdem,
			&est.CNPJDV,
			&est.CNPJCompleto,
			&est.IdentificadorMatrizFilial,
			&est.NomeFantasia,
			&est.SituacaoCadastral,
			&est.DataSituacaoCadastral,
			&est.MotivoSituacaoCadastral,
			&est.NomeCidadeExterior,
			&est.Pais,
			&est.DataInicioAtividade,
			&est.CNAEFiscalPrincipal,
			&est.CNAEFiscalSecundaria,
			&est.TipoLogradouro,
			&est.Logradouro,
			&est.Numero,
			&est.Complemento,
			&est.Bairro,
			&est.CEP,
			&est.UF,
			&est.Municipio,
			&est.DDD1,
			&est.Telefone1,
			&est.DDD2,
			&est.Telefone2,
			&est.DDDFax,
			&est.Fax,
			&est.Email,
			&est.SituacaoEspecial,
			&est.DataSituacaoEspecial,
			&est.CreatedAt,
			&est.RazaoSocial,
			&est.CNAEDescricao,
			&est.MunicipioNome,
		)
		if err != nil {
			return nil, 0, fmt.Errorf("failed to scan estabelecimento: %w", err)
		}
		estabelecimentos = append(estabelecimentos, est)
	}
	if err := rows.Err(); err != nil {
		return nil, 0, fmt.Errorf("failed iterating estabelecimentos rows: %w", err)
	}

	if skipCount {
		total = fuzzySearchTotal(filters.Offset, filters.Limit, int64(len(estabelecimentos)))
		if int64(len(estabelecimentos)) > int64(filters.Limit) {
			estabelecimentos = estabelecimentos[:filters.Limit]
		}
	}

	return estabelecimentos, total, nil
}

func (r *EstabelecimentoRepository) GetByCNPJCompleto(
	ctx context.Context,
	cnpjCompleto string,
) (*models.EstabelecimentoCompleto, error) {
	query := `
		SELECT 
			e.id, e.uuid_id, e.cnpj_basico, e.cnpj_ordem, e.cnpj_dv, e.cnpj_completo,
			e.identificador_matriz_filial, e.nome_fantasia, e.situacao_cadastral,
			e.data_situacao_cadastral, e.motivo_situacao_cadastral,
			e.nome_cidade_exterior, e.pais, e.data_inicio_atividade,
			e.cnae_fiscal_principal, e.cnae_fiscal_secundaria,
			e.tipo_logradouro, e.logradouro, e.numero, e.complemento,
			e.bairro, e.cep, e.uf, e.municipio,
			e.ddd_1, e.telefone_1, e.ddd_2, e.telefone_2,
			e.ddd_fax, e.fax, e.email, e.situacao_especial,
			e.data_situacao_especial, e.created_at,
			emp.razao_social,
			c.descricao as cnae_descricao,
			m.descricao as municipio_nome
		FROM estabelecimentos e
		INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
		LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
		LEFT JOIN municipios m ON e.municipio = m.codigo
		WHERE e.cnpj_completo = $1
	`

	var est models.EstabelecimentoCompleto
	err := r.db.QueryRowContext(ctx, query, cnpjCompleto).Scan(
		&est.ID,
		&est.UUIDID,
		&est.CNPJBasico,
		&est.CNPJOrdem,
		&est.CNPJDV,
		&est.CNPJCompleto,
		&est.IdentificadorMatrizFilial,
		&est.NomeFantasia,
		&est.SituacaoCadastral,
		&est.DataSituacaoCadastral,
		&est.MotivoSituacaoCadastral,
		&est.NomeCidadeExterior,
		&est.Pais,
		&est.DataInicioAtividade,
		&est.CNAEFiscalPrincipal,
		&est.CNAEFiscalSecundaria,
		&est.TipoLogradouro,
		&est.Logradouro,
		&est.Numero,
		&est.Complemento,
		&est.Bairro,
		&est.CEP,
		&est.UF,
		&est.Municipio,
		&est.DDD1,
		&est.Telefone1,
		&est.DDD2,
		&est.Telefone2,
		&est.DDDFax,
		&est.Fax,
		&est.Email,
		&est.SituacaoEspecial,
		&est.DataSituacaoEspecial,
		&est.CreatedAt,
		&est.RazaoSocial,
		&est.CNAEDescricao,
		&est.MunicipioNome,
	)

	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get estabelecimento: %w", err)
	}

	return &est, nil
}

// ExportToCSV streams estabelecimento rows as semicolon-delimited CSV via parameterized query.
//
//nolint:gocritic // Export SQL builder uses controlled column whitelist.
func (r *EstabelecimentoRepository) ExportToCSV(
	ctx context.Context,
	w io.Writer,
	filters models.SearchFilters,
	columns []string,
) error {
	columnMap := map[string]string{
		"cnpj_completo":         "e.cnpj_completo",
		"cnpj_basico":           "e.cnpj_basico",
		"nome_fantasia":         "COALESCE(e.nome_fantasia, '')",
		"razao_social":          "COALESCE(emp.razao_social, '')",
		"cnae_fiscal_principal": "COALESCE(e.cnae_fiscal_principal, '')",
		"cnae_descricao":        "COALESCE(c.descricao, '')",
		"uf":                    "COALESCE(e.uf, '')",
		"municipio":             "COALESCE(e.municipio, '')",
		"municipio_nome":        "COALESCE(m.descricao, '')",
		"situacao_cadastral":    "COALESCE(e.situacao_cadastral, '')",
		"logradouro":            "COALESCE(e.logradouro, '')",
		"numero":                "COALESCE(e.numero, '')",
		"bairro":                "COALESCE(e.bairro, '')",
		"cep":                   "COALESCE(e.cep, '')",
	}

	exportCols := columns
	if len(exportCols) == 0 {
		exportCols = []string{
			"cnpj_completo", "nome_fantasia", "razao_social",
			"cnae_fiscal_principal", "uf", "municipio",
		}
	}

	selectCols := make([]string, 0, len(exportCols))
	for _, col := range exportCols {
		sqlCol, ok := columnMap[col]
		if !ok {
			continue
		}
		selectCols = append(selectCols, textSelectExpr(sqlCol, col))
	}
	if len(selectCols) == 0 {
		return fmt.Errorf("no valid export columns selected")
	}

	whereParts := []string{"1=1"}
	args := []interface{}{}
	argPos := 1

	if filters.CNPJCompleto != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.cnpj_completo = $%d", argPos))
		args = append(args, filters.CNPJCompleto)
		argPos++
	}
	if filters.CNPJBasico != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.cnpj_basico = $%d", argPos))
		args = append(args, filters.CNPJBasico)
		argPos++
	}
	if filters.NomeFantasia != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.nome_fantasia ILIKE $%d", argPos))
		args = append(args, "%"+filters.NomeFantasia+"%")
		argPos++
	}
	if filters.CNAEPrincipal != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.cnae_fiscal_principal = $%d", argPos))
		args = append(args, filters.CNAEPrincipal)
		argPos++
	}
	if filters.UF != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.uf = $%d", argPos))
		args = append(args, filters.UF)
		argPos++
	}
	if filters.Municipio != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.municipio = $%d", argPos))
		args = append(args, filters.Municipio)
		argPos++
	}
	if filters.SituacaoCadastral != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.situacao_cadastral = $%d", argPos))
		args = append(args, filters.SituacaoCadastral)
		argPos++
	}
	if filters.CEP != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.cep = $%d", argPos))
		args = append(args, filters.CEP)
		argPos++
	}

	limit := filters.Limit
	if limit <= 0 {
		limit = 10000
	}

	fromClause := `
		FROM estabelecimentos e
		INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
		LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
		LEFT JOIN municipios m ON e.municipio = m.codigo`

	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	query := fmt.Sprintf(
		"SELECT %s %s WHERE %s ORDER BY e.nome_fantasia LIMIT $%d",
		strings.Join(selectCols, ", "),
		fromClause,
		strings.Join(whereParts, " AND "),
		argPos,
	)
	args = append(args, limit)

	return streamCSV(ctx, r.db, w, query, args, exportCols)
}
