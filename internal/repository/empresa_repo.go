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

//nolint:gocritic,misspell,cyclop // value arg; Receita fields; explicit SQL filter matrix.
func (r *EmpresaRepository) SearchEmpresas(
	ctx context.Context,
	filters models.SearchFilters,
) ([]models.Empresa, PageMeta, error) {
	baseSelect := `
		SELECT 
			uuid_id, cnpj_basico, razao_social, natureza_juridica,
			qualificacao_responsavel, capital_social, porte_empresa,
			ente_federativo_responsavel, created_at, updated_at`

	// #nosec G202 -- static SQL fragments; placeholders use internal arg counters.
	query := baseSelect + `
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

	razaoMode := textSearchNone
	razaoSocialPos := 0
	if filters.RazaoSocial != "" {
		razaoMode = detectTextSearchMode(filters.RazaoSocial)
		razaoSocialPos = argPos
		switch razaoMode {
		case textSearchFTS:
			query += ftsRazaoSocialWhere(argPos)
		case textSearchTrigram:
			query += fuzzyRazaoSocialWhere(argPos)
		case textSearchNone:
		}
		args = append(args, filters.RazaoSocial)
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

	keyset := useKeysetPagination(filters.Cursor, filters.Offset)
	skipCount := hasFuzzyTextFilter(filters)
	if filters.Limit <= 0 {
		filters.Limit = 100
	}
	queryLimit := filters.Limit
	if skipCount || keyset {
		queryLimit = filters.Limit + 1
	}

	if keyset {
		var clause string
		var err error
		switch razaoMode {
		case textSearchFTS:
			clause, err = empresaFTSKeysetClause(filters.Cursor, razaoSocialPos, &argPos, &args)
		case textSearchTrigram:
			clause, err = empresaKeysetClause(filters.Cursor, razaoSocialPos, true, &argPos, &args)
		case textSearchNone:
			clause, err = empresaKeysetClause(filters.Cursor, 0, false, &argPos, &args)
		}
		if err != nil {
			return nil, PageMeta{}, fmt.Errorf("invalid cursor: %w", err)
		}
		query += clause
	}

	var total int64
	if !skipCount && !keyset {
		countQuery := "SELECT COUNT(*) FROM (" + strings.Replace(query, baseSelect, "SELECT 1", 1) + ") AS count_query"
		if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
			return nil, PageMeta{}, fmt.Errorf("failed to count empresas: %w", err)
		}
	}

	orderBy := "cnpj_basico ASC"
	switch razaoMode {
	case textSearchFTS:
		orderBy = ftsRazaoSocialOrder(razaoSocialPos)
	case textSearchTrigram:
		orderBy = fuzzyRazaoSocialOrder(razaoSocialPos)
	case textSearchNone:
		if filters.RazaoSocial == "" {
			orderBy = "cnpj_basico ASC"
		}
	}

	scoreSelect := razaoSocialScoreSelect(razaoSocialPos, razaoMode)
	query = strings.Replace(query, baseSelect, baseSelect+scoreSelect, 1)

	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	if keyset {
		query += fmt.Sprintf(" ORDER BY %s LIMIT $%d", orderBy, argPos)
		args = append(args, queryLimit)
	} else {
		query += fmt.Sprintf(" ORDER BY %s LIMIT $%d OFFSET $%d", orderBy, argPos, argPos+1)
		args = append(args, queryLimit, filters.Offset)
	}

	rows, err := r.db.QueryContext(ctx, query, args...)
	if err != nil {
		return nil, PageMeta{}, fmt.Errorf("failed to query empresas: %w", err)
	}
	defer rows.Close()

	trackScore := razaoMode != textSearchNone
	var empresas = make([]models.Empresa, 0)
	var lastScore float64
	for rows.Next() {
		var emp models.Empresa
		var score float64
		scanArgs := []interface{}{
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
		}
		if trackScore {
			scanArgs = append(scanArgs, &score)
		}
		if err := rows.Scan(scanArgs...); err != nil {
			return nil, PageMeta{}, fmt.Errorf("failed to scan empresa: %w", err)
		}
		lastScore = score
		empresas = append(empresas, emp)
	}
	if err := rows.Err(); err != nil {
		return nil, PageMeta{}, fmt.Errorf("failed iterating empresas rows: %w", err)
	}

	meta := PageMeta{Total: total}
	fetched := int64(len(empresas))
	if skipCount || keyset {
		meta.HasMore = fetched > int64(filters.Limit)
		if meta.HasMore {
			empresas = empresas[:filters.Limit]
		}
		if skipCount {
			meta.Total = fuzzySearchTotal(filters.Offset, filters.Limit, fetched)
		}
	} else {
		meta.HasMore = filters.Offset+filters.Limit < int(total)
	}

	if meta.HasMore && len(empresas) > 0 {
		last := empresas[len(empresas)-1]
		switch razaoMode {
		case textSearchFTS, textSearchTrigram:
			meta.NextCursor = buildScoreCNPJCursor(lastScore, last.CNPJBasico)
		case textSearchNone:
			meta.NextCursor = buildCNPJCursor(last.CNPJBasico)
		}
	}

	return empresas, meta, nil
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
