package repository

import (
	"context"
	"database/sql"
	"fmt"
	"io"
	"strings"

	"busca-cnpj-2026/internal/exportcategory"
	"busca-cnpj-2026/internal/models"
)

const phoneFromClause = `
	FROM estabelecimentos e
	INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
	LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
	LEFT JOIN municipios m ON e.municipio = m.codigo`

var phoneCSVCols = []string{
	"cnpj_completo", "razao_social", "nome_fantasia", "telefone_1", "telefone_2",
	"email", "municipio_nome", "uf", "cnae_fiscal_principal", "cnae_descricao",
}

// ExportPhones streams phone contacts as CSV or optimized TXT.
func (r *EstabelecimentoRepository) ExportPhones(
	ctx context.Context,
	w io.Writer,
	req models.PhoneExportRequest,
) error {
	query, args, err := buildPhoneExportQuery(req)
	if err != nil {
		return err
	}
	if strings.EqualFold(req.Format, "txt") {
		return streamPhonesTXT(ctx, r.db, w, query, args)
	}
	return streamPhoneCSV(ctx, r.db, w, query, args)
}

func buildPhoneExportQuery(req models.PhoneExportRequest) (string, []any, error) {
	whereParts := []string{"NULLIF(TRIM(e.telefone_1), '') IS NOT NULL"}
	args := make([]any, 0, 12)
	argPos := 1

	if req.OnlyActive == nil || *req.OnlyActive {
		whereParts = append(whereParts, "e.situacao_cadastral IN ('2', '02')")
	}
	if req.Category != "" {
		category, ok := exportcategory.Find(req.Category)
		if !ok {
			return "", nil, fmt.Errorf("%w: %s", ErrUnknownCategory, req.Category)
		}
		whereParts = append(whereParts, exportcategory.MatchSQL(category, &argPos, &args))
	}
	if req.CNAEPrincipal != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.cnae_fiscal_principal = $%d", argPos))
		args = append(args, req.CNAEPrincipal)
		argPos++
	}
	if req.UF != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.uf = $%d", argPos))
		args = append(args, req.UF)
		argPos++
	}
	if req.Municipio != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.municipio = $%d", argPos))
		args = append(args, req.Municipio)
		argPos++
	}
	if req.MunicipioNome != "" {
		whereParts = append(whereParts, fmt.Sprintf("m.descricao ILIKE $%d", argPos))
		args = append(args, "%"+req.MunicipioNome+"%")
		argPos++
	}
	if req.NomeFantasia != "" {
		whereParts = append(whereParts, fmt.Sprintf("e.nome_fantasia ILIKE $%d", argPos))
		args = append(args, "%"+req.NomeFantasia+"%")
		argPos++
	}
	var err error
	whereParts, args, argPos, err = appendPhoneDateFilters(whereParts, args, argPos, req)
	if err != nil {
		return "", nil, err
	}
	if !hasPhoneExportFilter(req) {
		return "", nil, ErrPhoneFilterRequired
	}

	limitClause, limitArgs, _ := buildPhoneLimitClause(req, argPos)
	args = append(args, limitArgs...)

	selectList := strings.Join([]string{
		"e.cnpj_completo", "COALESCE(emp.razao_social, '')", "COALESCE(e.nome_fantasia, '')",
		"COALESCE(e.ddd_1, '')", "COALESCE(e.telefone_1, '')", "COALESCE(e.ddd_2, '')",
		"COALESCE(e.telefone_2, '')", "COALESCE(e.email, '')", "COALESCE(m.descricao, '')",
		"COALESCE(e.uf, '')", "COALESCE(e.cnae_fiscal_principal, '')", "COALESCE(c.descricao, '')",
	}, ", ")

	// #nosec G202 -- placeholders are generated from internal counters, not user input.
	query := fmt.Sprintf(
		"SELECT %s %s WHERE %s%s%s",
		selectList, phoneFromClause, joinPhoneWhere(whereParts), phoneExportOrderBy(), limitClause,
	)
	return query, args, nil
}

func hasPhoneExportFilter(req models.PhoneExportRequest) bool {
	if req.Category != "" || req.CNAEPrincipal != "" || req.NomeFantasia != "" {
		return true
	}
	return req.UF != "" || req.Municipio != "" || req.MunicipioNome != ""
}

func normalizePhoneLimit(limit int) int {
	if limit <= 0 {
		return 5000
	}
	if limit > 50000 {
		return 50000
	}
	return limit
}

func streamPhoneCSV(ctx context.Context, db *sql.DB, w io.Writer, query string, args []any) error {
	if _, err := w.Write([]byte(strings.Join(phoneCSVCols, ";") + "\n")); err != nil {
		return err
	}
	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return fmt.Errorf("failed to query phone export: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		line, err := scanPhoneRow(rows)
		if err != nil {
			return err
		}
		csvLine := strings.Join([]string{
			line.cnpj, line.razao, line.fantasia, line.phone1, line.phone2,
			line.email, line.cidade, line.uf, line.cnae, line.cnaeDesc,
		}, ";") + "\n"
		if _, err := w.Write([]byte(csvLine)); err != nil {
			return err
		}
	}
	return rows.Err()
}

func streamPhonesTXT(ctx context.Context, db *sql.DB, w io.Writer, query string, args []any) error {
	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return fmt.Errorf("failed to query phone export: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		line, err := scanPhoneRow(rows)
		if err != nil {
			return err
		}
		name := line.fantasia
		if name == "" {
			name = line.razao
		}
		if err := writeTXTPhone(w, line.phone1, name, line.cidade, line.uf, line.cnaeDesc); err != nil {
			return err
		}
		if line.phone2 != "" && line.phone2 != line.phone1 {
			if err := writeTXTPhone(w, line.phone2, name, line.cidade, line.uf, line.cnaeDesc); err != nil {
				return err
			}
		}
	}
	return rows.Err()
}

type phoneRow struct {
	cnpj, razao, fantasia, phone1, phone2, email, cidade, uf, cnae, cnaeDesc string
}

func scanPhoneRow(rows *sql.Rows) (phoneRow, error) {
	var (
		ddd1, tel1, ddd2, tel2 string
		line                   phoneRow
	)
	if err := rows.Scan(
		&line.cnpj, &line.razao, &line.fantasia, &ddd1, &tel1, &ddd2, &tel2,
		&line.email, &line.cidade, &line.uf, &line.cnae, &line.cnaeDesc,
	); err != nil {
		return phoneRow{}, fmt.Errorf("failed to scan phone row: %w", err)
	}
	line.phone1 = formatPhone(ddd1, tel1)
	line.phone2 = formatPhone(ddd2, tel2)
	return line, nil
}

func writeTXTPhone(w io.Writer, phone, name, city, uf, cnaeDesc string) error {
	if phone == "" {
		return nil
	}
	_, err := fmt.Fprintf(w, "%s | %s | %s/%s | %s\n", phone, name, city, uf, cnaeDesc)
	return err
}

func formatPhone(ddd, number string) string {
	digits := strings.TrimSpace(ddd) + strings.TrimSpace(number)
	digits = strings.Map(func(r rune) rune {
		if r >= '0' && r <= '9' {
			return r
		}
		return -1
	}, digits)
	if len(digits) < 10 {
		return ""
	}
	return digits
}
