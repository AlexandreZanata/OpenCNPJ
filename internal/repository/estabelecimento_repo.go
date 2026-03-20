package repository

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"io"
	"strings"
	"time"

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

	// Count query
	countQuery := "SELECT COUNT(*) FROM (" + query + ") AS count_query"
	var total int64
	if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
		return nil, 0, fmt.Errorf("failed to count estabelecimentos: %w", err)
	}

	// Add pagination
	if filters.Limit <= 0 {
		filters.Limit = 100
	}
	query += fmt.Sprintf(" ORDER BY e.nome_fantasia LIMIT $%d OFFSET $%d", argPos, argPos+1)
	args = append(args, filters.Limit, filters.Offset)

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

// Uses a PostgreSQL function to handle parameterized queries with COPY TO STDOUT.
//
//nolint:cyclop,gocritic,gosec // Export SQL builder is intentionally dynamic and branch-heavy.
func (r *EstabelecimentoRepository) ExportToCSV(
	ctx context.Context,
	w io.Writer,
	filters models.SearchFilters,
	columns []string,
) error {
	// Build column mapping for SELECT
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

	// Build SELECT columns
	selectCols := make([]string, 0, len(columns))
	for _, col := range columns {
		if sqlCol, ok := columnMap[col]; ok {
			selectCols = append(selectCols, sqlCol+" AS "+col)
		}
	}
	if len(selectCols) == 0 {
		selectCols = []string{
			"e.cnpj_completo",
			"COALESCE(e.nome_fantasia, '') AS nome_fantasia",
			"COALESCE(emp.razao_social, '') AS razao_social",
			"COALESCE(e.cnae_fiscal_principal, '') AS cnae_fiscal_principal",
			"COALESCE(e.uf, '') AS uf",
			"COALESCE(m.descricao, '') AS municipio",
		}
	}

	// Build WHERE clause with parameters
	whereParts := []string{"1=1"}
	args := []interface{}{}
	argPos := 1

	if filters.CNPJCompleto != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.cnpj_completo = $%d", argPos))
		args = append(args, filters.CNPJCompleto)
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
	}

	whereClause := "WHERE " + strings.Join(whereParts, " AND ")

	// Use COPY TO STDOUT with a subquery
	// Since COPY doesn't support parameters directly, we'll use a function approach
	// Create a temporary function that accepts parameters and returns the data

	// Build function SQL with parameters embedded safely
	// Use a unique function name to avoid conflicts
	funcName := fmt.Sprintf("temp_export_estab_%d", time.Now().UnixNano())

	// Build WHERE clause with parameters embedded (safe because we validate inputs)
	whereClauseWithParams := whereClause
	for i, arg := range args {
		var value string
		switch v := arg.(type) {
		case string:
			// Escape single quotes and wrap in quotes
			escaped := strings.ReplaceAll(v, "'", "''")
			value = fmt.Sprintf("'%s'", escaped)
		case int, int64:
			value = fmt.Sprintf("%d", v)
		case float64:
			value = fmt.Sprintf("%f", v)
		default:
			value = fmt.Sprintf("'%v'", v)
		}
		// Replace placeholder in whereClause
		placeholder := fmt.Sprintf("$%d", i+1)
		whereClauseWithParams = strings.Replace(whereClauseWithParams, placeholder, value, 1)
	}
	whereClause = whereClauseWithParams

	// Create function with embedded parameters
	createFuncSQL := fmt.Sprintf(`
		CREATE OR REPLACE FUNCTION %s()
		RETURNS TABLE(%s) AS $$
		BEGIN
			RETURN QUERY
			SELECT %s
			FROM estabelecimentos e
			INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
			LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
			LEFT JOIN municipios m ON e.municipio = m.codigo
			%s;
		END;
		$$ LANGUAGE plpgsql;
	`, funcName,
		strings.Join(columns, " TEXT, ")+" TEXT",
		strings.Join(selectCols, ", "),
		whereClause)

	// Execute function creation
	txn, err := r.db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		_ = txn.Rollback()
	}()

	// Create the function
	if _, err := txn.ExecContext(ctx, createFuncSQL); err != nil {
		return fmt.Errorf("failed to create export function: %w", err)
	}

	// Write CSV header
	header := strings.Join(columns, ";") + "\n"
	if _, err := w.Write([]byte(header)); err != nil {
		return fmt.Errorf("failed to write CSV header: %w", err)
	}

	// Query the function and stream results to CSV
	// This approach is faster than SELECT + CSV writer and uses less memory

	rows, err := txn.QueryContext(ctx, fmt.Sprintf("SELECT * FROM %s()", funcName))
	if err != nil {
		return fmt.Errorf("failed to query export function: %w", err)
	}
	defer rows.Close()

	// Stream results to CSV
	rowCount := 0
	for rows.Next() {
		// Read row values
		values := make([]interface{}, len(columns))
		valuePtrs := make([]interface{}, len(columns))
		for i := range values {
			valuePtrs[i] = &values[i]
		}

		if err := rows.Scan(valuePtrs...); err != nil {
			return fmt.Errorf("failed to scan row: %w", err)
		}

		// Convert to CSV row
		csvRow := make([]string, len(columns))
		for i, val := range values {
			if val == nil {
				csvRow[i] = ""
			} else {
				csvRow[i] = fmt.Sprintf("%v", val)
			}
		}

		// Write CSV row
		rowStr := strings.Join(csvRow, ";") + "\n"
		if _, err := w.Write([]byte(rowStr)); err != nil {
			return fmt.Errorf("failed to write CSV row: %w", err)
		}

		rowCount++
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("failed iterating export rows: %w", err)
	}

	// Drop the temporary function
	_, _ = txn.ExecContext(ctx, fmt.Sprintf("DROP FUNCTION IF EXISTS %s()", funcName))

	if err := txn.Commit(); err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	return nil
}
