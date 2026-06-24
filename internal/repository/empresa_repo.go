package repository

//nolint:misspell // Domain-specific Portuguese fields from Receita Federal schema.

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
			return nil, 0, fmt.Errorf("failed to count empresas: %w", err)
		}
	}

	// Add pagination
	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	query += fmt.Sprintf(" ORDER BY razao_social LIMIT $%d OFFSET $%d", argPos, argPos+1)
	args = append(args, queryLimit, filters.Offset)

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, 0, fmt.Errorf("failed to query empresas: %w", err)
	}
	defer rows.Close()

	var empresas = make([]models.Empresa, 0)
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

	if skipCount {
		total = fuzzySearchTotal(filters.Offset, filters.Limit, int64(len(empresas)))
		if int64(len(empresas)) > int64(filters.Limit) {
			empresas = empresas[:filters.Limit]
		}
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

// ExportToCSV streams empresa rows as semicolon-delimited CSV via parameterized query.
//
//nolint:gocritic,misspell // Export SQL builder and Receita field names.
func (r *EmpresaRepository) ExportToCSV(
	ctx context.Context,
	w io.Writer,
	filters models.SearchFilters,
	columns []string,
) error {
	columnMap := map[string]string{
		"cnpj_basico":                 "cnpj_basico",
		"razao_social":                "razao_social",
		"natureza_juridica":           "COALESCE(natureza_juridica, '')",
		"capital_social":              "COALESCE(capital_social::text, '')",
		"porte_empresa":               "COALESCE(porte_empresa, '')",
		"ente_federativo_responsavel": "COALESCE(ente_federativo_responsavel, '')", //nolint:misspell
	}

	selectCols := make([]string, 0, len(columns))
	exportCols := columns
	if len(exportCols) == 0 {
		exportCols = []string{"cnpj_basico", "razao_social", "natureza_juridica"}
	}
	for _, col := range exportCols {
		sqlCol, ok := columnMap[col]
		if !ok {
			continue
		}
		selectCols = append(selectCols, textSelectExpr(sqlCol, col))
	}
	if len(selectCols) == 0 {
		return ErrNoValidExportColumns
	}

	whereParts := []string{"1=1"}
	args := []interface{}{}
	argPos := 1

	if filters.CNPJBasico != "" {
		whereParts = append(whereParts, fmt.Sprintf("cnpj_basico = $%d", argPos))
		args = append(args, filters.CNPJBasico)
		argPos++
	}
	if filters.RazaoSocial != "" {
		whereParts = append(whereParts, fmt.Sprintf("razao_social ILIKE $%d", argPos))
		args = append(args, "%"+filters.RazaoSocial+"%")
		argPos++
	}
	if filters.NaturezaJuridica != "" {
		whereParts = append(whereParts, fmt.Sprintf("natureza_juridica = $%d", argPos))
		args = append(args, filters.NaturezaJuridica)
		argPos++
	}
	if filters.PorteEmpresa != "" {
		whereParts = append(whereParts, fmt.Sprintf("porte_empresa = $%d", argPos))
		args = append(args, filters.PorteEmpresa)
		argPos++
	}

	limit := filters.Limit
	if limit <= 0 {
		limit = 10000
	}

	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	query := fmt.Sprintf(
		"SELECT %s FROM empresas WHERE %s ORDER BY razao_social LIMIT $%d",
		strings.Join(selectCols, ", "),
		strings.Join(whereParts, " AND "),
		argPos,
	)
	args = append(args, limit)

	return streamCSV(ctx, r.db, w, query, args, exportCols)
}
