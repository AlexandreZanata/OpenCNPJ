package repository

import (
	"fmt"
	"regexp"
	"strings"
	"time"

	"busca-cnpj-2026/internal/models"
)

var exportDatePattern = regexp.MustCompile(`^\d{4}-\d{2}-\d{2}$`)

func appendPhoneDateFilters(
	whereParts []string,
	args []any,
	argPos int,
	req models.PhoneExportRequest,
) ([]string, []any, int, error) {
	if req.CreatedFrom != "" {
		from, err := parseExportDate(req.CreatedFrom)
		if err != nil {
			return whereParts, args, argPos, err
		}
		whereParts = append(whereParts, fmt.Sprintf("e.data_inicio_atividade >= $%d", argPos))
		args = append(args, from)
		argPos++
	}
	if req.CreatedTo != "" {
		to, err := parseExportDate(req.CreatedTo)
		if err != nil {
			return whereParts, args, argPos, err
		}
		whereParts = append(whereParts, fmt.Sprintf("e.data_inicio_atividade <= $%d", argPos))
		args = append(args, to)
		argPos++
	}
	return whereParts, args, argPos, nil
}

func parseExportDate(value string) (time.Time, error) {
	if !exportDatePattern.MatchString(value) {
		return time.Time{}, fmt.Errorf("%w: %q", ErrInvalidExportDate, value)
	}
	parsed, err := time.Parse("2006-01-02", value)
	if err != nil {
		return time.Time{}, fmt.Errorf("%w: %q", ErrInvalidExportDate, value)
	}
	return parsed, nil
}

func buildPhoneLimitClause(req models.PhoneExportRequest, argPos int) (string, []any, int) {
	if req.ExportAll != nil && *req.ExportAll {
		return "", nil, argPos
	}
	limit := normalizePhoneLimit(req.Limit)
	clause := fmt.Sprintf(" LIMIT $%d", argPos)
	return clause, []any{limit}, argPos + 1
}

func phoneExportOrderBy() string {
	return " ORDER BY e.uf, m.descricao, emp.razao_social"
}

func joinPhoneWhere(parts []string) string {
	return strings.Join(parts, " AND ")
}
