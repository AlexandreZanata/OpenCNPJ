package repository

//nolint:misspell // Domain-specific Portuguese fields from Receita Federal schema.

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

type EmpresaRepository struct {
	db *sql.DB
}

func NewEmpresaRepository() *EmpresaRepository {
	return &EmpresaRepository{
		db: database.DB,
	}
}

//nolint:gocritic,misspell // Keeping value argument and Receita field names.
func (r *EmpresaRepository) SearchEmpresas(
	ctx context.Context,
	filters models.SearchFilters,
) ([]models.Empresa, int64, error) {
	query := `
		SELECT 
			uuid_id, cnpj_basico, razao_social, natureza_juridica,
			qualificacao_responsavel, capital_social, porte_empresa,
			ente_federativo_responsavel, created_at, updated_at
		FROM empresas
		WHERE 1=1
	`

	args := []interface{}{}
	argPos := 1

	if filters.CNPJBasico != "" {
		query += fmt.Sprintf(" AND cnpj_basico = $%d", argPos)
		args = append(args, filters.CNPJBasico)
		argPos++
	}
	if filters.UUIDID != "" {
		query += fmt.Sprintf(" AND uuid_id = $%d", argPos)
		args = append(args, filters.UUIDID)
		argPos++
	}

	if filters.RazaoSocial != "" {
		query += fmt.Sprintf(" AND razao_social ILIKE $%d", argPos)
		args = append(args, "%"+filters.RazaoSocial+"%")
		argPos++
	}

	if filters.NaturezaJuridica != "" {
		query += fmt.Sprintf(" AND natureza_juridica = $%d", argPos)
		args = append(args, filters.NaturezaJuridica)
		argPos++
	}

	if filters.PorteEmpresa != "" {
		query += fmt.Sprintf(" AND porte_empresa = $%d", argPos)
		args = append(args, filters.PorteEmpresa)
		argPos++
	}

	if filters.CapitalSocialMin != nil {
		query += fmt.Sprintf(" AND capital_social >= $%d", argPos)
		args = append(args, *filters.CapitalSocialMin)
		argPos++
	}

	if filters.CapitalSocialMax != nil {
		query += fmt.Sprintf(" AND capital_social <= $%d", argPos)
		args = append(args, *filters.CapitalSocialMax)
		argPos++
	}

	// Count query
	countQuery := "SELECT COUNT(*) FROM (" + query + ") AS count_query"
	var total int64
	if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
		return nil, 0, fmt.Errorf("failed to count empresas: %w", err)
	}

	// Add pagination
	if filters.Limit <= 0 {
		filters.Limit = 100
	}
	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	query += fmt.Sprintf(" ORDER BY razao_social LIMIT $%d OFFSET $%d", argPos, argPos+1)
	args = append(args, filters.Limit, filters.Offset)

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, 0, fmt.Errorf("failed to query empresas: %w", err)
	}
	defer rows.Close()

	var empresas []models.Empresa
	for rows.Next() {
		var emp models.Empresa
		err := rows.Scan(
			&emp.UUIDID,
			&emp.CNPJBasico,
			&emp.RazaoSocial,
			&emp.NaturezaJuridica,
			&emp.QualificacaoResponsavel,
			&emp.CapitalSocial,
			&emp.PorteEmpresa,
			&emp.EnteFederativoResponsavel,
			&emp.CreatedAt,
			&emp.UpdatedAt,
		)
		if err != nil {
			return nil, 0, fmt.Errorf("failed to scan empresa: %w", err)
		}
		empresas = append(empresas, emp)
	}
	if err := rows.Err(); err != nil {
		return nil, 0, fmt.Errorf("failed iterating empresas rows: %w", err)
	}

	return empresas, total, nil
}

//nolint:misspell // Receita field naming.
func (r *EmpresaRepository) GetByCNPJBasico(ctx context.Context, cnpjBasico string) (*models.Empresa, error) {
	query := `
		SELECT 
			uuid_id, cnpj_basico, razao_social, natureza_juridica,
			qualificacao_responsavel, capital_social, porte_empresa,
			ente_federativo_responsavel, created_at, updated_at
		FROM empresas
		WHERE cnpj_basico = $1
	`

	var emp models.Empresa
	err := r.db.QueryRowContext(ctx, query, cnpjBasico).Scan(
		&emp.UUIDID,
		&emp.CNPJBasico,
		&emp.RazaoSocial,
		&emp.NaturezaJuridica,
		&emp.QualificacaoResponsavel,
		&emp.CapitalSocial,
		&emp.PorteEmpresa,
		&emp.EnteFederativoResponsavel,
		&emp.CreatedAt,
		&emp.UpdatedAt,
	)

	if errors.Is(err, sql.ErrNoRows) {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("failed to get empresa: %w", err)
	}

	return &emp, nil
}

// This is faster than SELECT + CSV writer and uses less memory.
//
//nolint:cyclop,gocritic,gosec,misspell // Export SQL builder and Receita field names.
func (r *EmpresaRepository) ExportToCSV(
	ctx context.Context,
	w io.Writer,
	filters models.SearchFilters,
	columns []string,
) error {
	// Build column mapping
	columnMap := map[string]string{
		"cnpj_basico":                 "cnpj_basico",
		"razao_social":                "razao_social",
		"natureza_juridica":           "COALESCE(natureza_juridica, '')",
		"capital_social":              "COALESCE(capital_social::text, '')",
		"porte_empresa":               "COALESCE(porte_empresa, '')",
		"ente_federativo_responsavel": "COALESCE(ente_federativo_responsavel, '')", //nolint:misspell
	}

	// Build SELECT columns
	selectCols := make([]string, 0, len(columns))
	for _, col := range columns {
		if sqlCol, ok := columnMap[col]; ok {
			selectCols = append(selectCols, sqlCol+" AS "+col)
		}
	}
	if len(selectCols) == 0 {
		selectCols = []string{"cnpj_basico", "razao_social", "COALESCE(natureza_juridica, '') AS natureza_juridica"}
	}

	// Build WHERE clause
	whereParts := []string{"1=1"}
	args := []interface{}{}

	if filters.CNPJBasico != "" {
		args = append(args, filters.CNPJBasico)
		whereParts = append(whereParts, fmt.Sprintf("cnpj_basico = $%d", len(args)))
	}
	if filters.RazaoSocial != "" {
		args = append(args, "%"+filters.RazaoSocial+"%")
		whereParts = append(whereParts, fmt.Sprintf("razao_social ILIKE $%d", len(args)))
	}
	if filters.NaturezaJuridica != "" {
		args = append(args, filters.NaturezaJuridica)
		whereParts = append(whereParts, fmt.Sprintf("natureza_juridica = $%d", len(args)))
	}
	if filters.PorteEmpresa != "" {
		args = append(args, filters.PorteEmpresa)
		whereParts = append(whereParts, fmt.Sprintf("porte_empresa = $%d", len(args)))
	}

	whereClause := "WHERE " + strings.Join(whereParts, " AND ")

	// Build function SQL with parameters embedded
	funcName := fmt.Sprintf("temp_export_emp_%d", time.Now().UnixNano())

	// Embed parameters safely
	whereClauseWithParams := whereClause
	for i, arg := range args {
		var value string
		switch v := arg.(type) {
		case string:
			escaped := strings.ReplaceAll(v, "'", "''")
			value = fmt.Sprintf("'%s'", escaped)
		default:
			value = fmt.Sprintf("'%v'", v)
		}
		placeholder := fmt.Sprintf("$%d", i+1)
		whereClauseWithParams = strings.Replace(whereClauseWithParams, placeholder, value, 1)
	}
	whereClause = whereClauseWithParams

	// #nosec G201 -- dynamic SQL is built from controlled column whitelist and escaped parameters.
	createFuncSQL := fmt.Sprintf(`
		CREATE OR REPLACE FUNCTION %s()
		RETURNS TABLE(%s) AS $$
		BEGIN
			RETURN QUERY
			SELECT %s
			FROM empresas
			%s;
		END;
		$$ LANGUAGE plpgsql;
	`, funcName,
		strings.Join(columns, " TEXT, ")+" TEXT",
		strings.Join(selectCols, ", "),
		whereClause)

	txn, err := r.db.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	defer func() {
		_ = txn.Rollback()
	}()

	if _, err := txn.ExecContext(ctx, createFuncSQL); err != nil {
		return fmt.Errorf("failed to create export function: %w", err)
	}

	// Write CSV header
	header := strings.Join(columns, ";") + "\n"
	if _, err := w.Write([]byte(header)); err != nil {
		return fmt.Errorf("failed to write CSV header: %w", err)
	}

	// Query function and stream results
	rows, err := txn.QueryContext(ctx, fmt.Sprintf("SELECT * FROM %s()", funcName))
	if err != nil {
		return fmt.Errorf("failed to query export function: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		values := make([]interface{}, len(columns))
		valuePtrs := make([]interface{}, len(columns))
		for i := range values {
			valuePtrs[i] = &values[i]
		}

		if err := rows.Scan(valuePtrs...); err != nil {
			return fmt.Errorf("failed to scan row: %w", err)
		}

		csvRow := make([]string, len(columns))
		for i, val := range values {
			if val == nil {
				csvRow[i] = ""
			} else {
				csvRow[i] = fmt.Sprintf("%v", val)
			}
		}

		rowStr := strings.Join(csvRow, ";") + "\n"
		if _, err := w.Write([]byte(rowStr)); err != nil {
			return fmt.Errorf("failed to write CSV row: %w", err)
		}
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("failed iterating export rows: %w", err)
	}

	_, _ = txn.ExecContext(ctx, fmt.Sprintf("DROP FUNCTION IF EXISTS %s()", funcName))

	if err := txn.Commit(); err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}

	return nil
}
