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

//nolint:gocritic,cyclop // value arg; explicit SQL filter matrix.
func (r *EstabelecimentoRepository) SearchEstabelecimentos(
	ctx context.Context,
	filters models.SearchFilters,
) ([]models.EstabelecimentoCompleto, PageMeta, error) {
	selectClause := "SELECT " + estabelecimentoCompletoSelect
	// #nosec G202 -- static SQL fragments; placeholders use internal arg counters.
	query := selectClause + estabelecimentoCompletoFrom + `
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

	nomeMode := textSearchNone
	nomeFantasiaPos := 0
	if filters.NomeFantasia != "" {
		nomeMode = detectTextSearchMode(filters.NomeFantasia)
		nomeFantasiaPos = argPos
		switch nomeMode {
		case textSearchFTS:
			query += ftsNomeFantasiaWhere(argPos)
		case textSearchTrigram:
			query += fuzzyNomeFantasiaWhere(argPos)
		case textSearchNone:
		}
		args = append(args, filters.NomeFantasia)
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
		switch nomeMode {
		case textSearchFTS:
			clause, err = estabelecimentoFTSKeysetClause(filters.Cursor, nomeFantasiaPos, &argPos, &args)
		case textSearchTrigram:
			clause, err = estabelecimentoKeysetClause(filters.Cursor, nomeFantasiaPos, true, &argPos, &args)
		case textSearchNone:
			clause, err = estabelecimentoKeysetClause(filters.Cursor, 0, false, &argPos, &args)
		}
		if err != nil {
			return nil, PageMeta{}, fmt.Errorf("invalid cursor: %w", err)
		}
		query += clause
	}

	var total int64
	if !skipCount && !keyset {
		countQuery := "SELECT COUNT(*) FROM (" + strings.Replace(query, selectClause, "SELECT 1", 1) + ") AS count_query"
		if err := r.db.QueryRowContext(ctx, countQuery, args...).Scan(&total); err != nil {
			return nil, PageMeta{}, fmt.Errorf("failed to count estabelecimentos: %w", err)
		}
	}

	orderBy := "e.id ASC"
	switch nomeMode {
	case textSearchFTS:
		orderBy = ftsNomeFantasiaOrder(nomeFantasiaPos)
	case textSearchTrigram:
		orderBy = fuzzyNomeFantasiaOrder(nomeFantasiaPos)
	case textSearchNone:
	}

	scoreSelect := nomeFantasiaScoreSelect(nomeFantasiaPos, nomeMode)
	query = strings.Replace(query, selectClause, selectClause+scoreSelect, 1)

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
		return nil, PageMeta{}, fmt.Errorf("failed to query estabelecimentos: %w", err)
	}
	defer rows.Close()

	trackScore := nomeMode != textSearchNone
	estabelecimentos := make([]models.EstabelecimentoCompleto, 0)
	var lastScore float64
	for rows.Next() {
		var est models.EstabelecimentoCompleto
		var score float64
		if trackScore {
			if err := scanEstabelecimentoCompletoWithScore(rows, &est, &score); err != nil {
				return nil, PageMeta{}, err
			}
			lastScore = score
		} else if err := scanEstabelecimentoCompleto(rows, &est); err != nil {
			return nil, PageMeta{}, err
		}
		estabelecimentos = append(estabelecimentos, est)
	}
	if err := rows.Err(); err != nil {
		return nil, PageMeta{}, fmt.Errorf("failed iterating estabelecimentos rows: %w", err)
	}

	meta := PageMeta{Total: total}
	fetched := int64(len(estabelecimentos))
	if skipCount || keyset {
		meta.HasMore = fetched > int64(filters.Limit)
		if meta.HasMore {
			estabelecimentos = estabelecimentos[:filters.Limit]
		}
		if skipCount {
			meta.Total = fuzzySearchTotal(filters.Offset, filters.Limit, fetched)
		}
	} else {
		meta.HasMore = filters.Offset+filters.Limit < int(total)
	}

	if meta.HasMore && len(estabelecimentos) > 0 {
		last := estabelecimentos[len(estabelecimentos)-1]
		switch nomeMode {
		case textSearchFTS, textSearchTrigram:
			meta.NextCursor = buildScoreIDCursor(lastScore, last.ID)
		case textSearchNone:
			meta.NextCursor = buildIDCursor(last.ID)
		}
	}

	return estabelecimentos, meta, nil
}

func (r *EstabelecimentoRepository) GetByCNPJCompleto(
	ctx context.Context,
	cnpjCompleto string,
) (*models.EstabelecimentoCompleto, error) {
	query := `
		SELECT ` + estabelecimentoCompletoSelect + estabelecimentoCompletoFrom + `
		WHERE e.cnpj_completo = $1
	`

	var est models.EstabelecimentoCompleto
	err := scanEstabelecimentoCompleto(r.db.QueryRowContext(ctx, query, cnpjCompleto), &est)

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
		return ErrNoValidExportColumns
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
