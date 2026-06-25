package repository

import (
	"context"
	"database/sql"
	"fmt"
	"strings"

	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/exportcategory"
	"busca-cnpj-2026/internal/models"
)

type LookupRepository struct {
	db *sql.DB
}

func NewLookupRepository() *LookupRepository {
	return &LookupRepository{db: database.DB}
}

func (r *LookupRepository) SearchSectors(ctx context.Context, query string, limit int) ([]models.LookupItem, error) {
	limit = normalizeLookupLimit(limit)
	term := normalizeLookupQuery(query)
	items := make([]models.LookupItem, 0, limit)

	presetCap := limit
	if term != "" {
		presetCap = minInt(limit, 5)
	}
	for _, preset := range exportcategory.SearchPresets(query, presetCap) {
		items = append(items, models.LookupItem{
			Type:        "preset",
			Code:        preset.Key,
			Label:       preset.Label,
			Description: preset.Description,
		})
	}

	if !cnaeLookupMinLen(term) || len(items) >= limit {
		return items, nil
	}

	cnaes, err := r.SearchCNAE(ctx, query, limit-len(items))
	if err != nil {
		return nil, err
	}
	items = append(items, cnaes...)
	return items, nil
}

func (r *LookupRepository) SearchCNAE(ctx context.Context, query string, limit int) ([]models.LookupItem, error) {
	limit = normalizeLookupLimit(limit)
	term := normalizeLookupQuery(query)
	if !cnaeLookupMinLen(term) {
		return nil, nil
	}

	terms := splitLookupTerms(term)
	args := make([]any, 0, len(terms)+3)
	argPos := 1

	codeArg := argPos
	args = append(args, term+"%")
	argPos++

	descMatch := buildCNAEDescricaoMatch(terms, &argPos, &args)
	simArg := argPos
	args = append(args, foldAccents(term))
	argPos++

	limitArg := argPos
	args = append(args, limit)

	foldedDesc := accentFoldExpr("descricao")
	// #nosec G201 -- SQL placeholders are integers from argPos, not user input.
	sql := fmt.Sprintf(`
		SELECT codigo, descricao
		FROM %s
		WHERE codigo LIKE $%d OR (%s)
		ORDER BY
		  CASE WHEN codigo LIKE $%d THEN 0 ELSE 1 END,
		  similarity(%s, $%d) DESC,
		  descricao
		LIMIT $%d
	`, tableLookupCNAE, codeArg, descMatch, codeArg, foldedDesc, simArg, limitArg)

	rows, err := r.db.QueryContext(ctx, sql, args...)
	if err != nil {
		return nil, fmt.Errorf("lookup cnae: %w", err)
	}
	defer rows.Close()

	return scanLookupRows(rows, "cnae")
}

func minInt(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func (r *LookupRepository) SearchMunicipios(
	ctx context.Context,
	query, uf string,
	limit int,
) ([]models.LookupItem, error) {
	limit = normalizeLookupLimit(limit)
	term := normalizeLookupQuery(query)
	if term == "" {
		return nil, nil
	}

	rows, err := r.db.QueryContext(ctx, fmt.Sprintf(`
		SELECT m.codigo, m.descricao, m.uf
		FROM %s m
		WHERE ($1 = '' OR m.uf = $1)
		  AND (m.codigo LIKE $2 OR m.descricao ILIKE $3)
		ORDER BY
		  CASE WHEN m.codigo LIKE $4 THEN 0 ELSE 1 END,
		  CASE WHEN m.uf IS NOT NULL AND m.uf <> '' THEN 0 ELSE 1 END,
		  m.descricao
		LIMIT $5
	`, tableLookupMunicip), uf, term+"%", "%"+term+"%", term+"%", limit)
	if err != nil {
		return nil, fmt.Errorf("lookup municipio: %w", err)
	}
	defer rows.Close()

	return scanMunicipioRows(rows)
}

func (r *LookupRepository) SearchNomeFantasia(
	ctx context.Context,
	query, uf string,
	limit int,
) ([]models.LookupItem, error) {
	limit = normalizeLookupLimit(limit)
	term := normalizeLookupQuery(query)
	if len(term) < 3 {
		return nil, nil
	}

	rows, err := r.db.QueryContext(ctx, `
		SELECT DISTINCT ON (nome_fantasia) COALESCE(nome_fantasia, ''), COALESCE(uf, '')
		FROM estabelecimentos
		WHERE nome_fantasia ILIKE $1
		  AND ($2 = '' OR uf = $2)
		  AND NULLIF(TRIM(nome_fantasia), '') IS NOT NULL
		ORDER BY nome_fantasia
		LIMIT $3
	`, "%"+term+"%", uf, limit)
	if err != nil {
		return nil, fmt.Errorf("lookup nome_fantasia: %w", err)
	}
	defer rows.Close()

	items := make([]models.LookupItem, 0, limit)
	for rows.Next() {
		var label, itemUF string
		if err := rows.Scan(&label, &itemUF); err != nil {
			return nil, fmt.Errorf("scan nome_fantasia: %w", err)
		}
		items = append(items, models.LookupItem{
			Type:  "nome_fantasia",
			Code:  label,
			Label: label,
			UF:    itemUF,
		})
	}
	return items, rows.Err()
}

func scanLookupRows(rows *sql.Rows, itemType string) ([]models.LookupItem, error) {
	items := make([]models.LookupItem, 0, 8)
	for rows.Next() {
		var code, desc string
		if err := rows.Scan(&code, &desc); err != nil {
			return nil, fmt.Errorf("scan lookup row: %w", err)
		}
		items = append(items, models.LookupItem{
			Type:        itemType,
			Code:        code,
			Label:       code + " — " + desc,
			Description: desc,
		})
	}
	return items, rows.Err()
}

func scanMunicipioRows(rows *sql.Rows) ([]models.LookupItem, error) {
	items := make([]models.LookupItem, 0, 8)
	for rows.Next() {
		var code, desc, uf string
		if err := rows.Scan(&code, &desc, &uf); err != nil {
			return nil, fmt.Errorf("scan municipio row: %w", err)
		}
		label := desc
		if uf != "" {
			label = desc + " (" + uf + ")"
		}
		items = append(items, models.LookupItem{
			Type:        "municipio",
			Code:        code,
			Label:       label,
			Description: desc,
			UF:          uf,
		})
	}
	return items, rows.Err()
}

func normalizeLookupLimit(limit int) int {
	if limit <= 0 {
		return 15
	}
	if limit > 100 {
		return 100
	}
	return limit
}

func normalizeLookupQuery(query string) string {
	return strings.TrimSpace(query)
}
