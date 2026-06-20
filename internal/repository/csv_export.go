package repository

import (
	"context"
	"database/sql"
	"fmt"
	"io"
	"strings"

	"busca-cnpj-2026/internal/models"
)

func textSelectExpr(sqlExpr, alias string) string {
	return fmt.Sprintf("(%s)::TEXT AS %s", sqlExpr, alias)
}

func hasFuzzyTextFilter(filters models.SearchFilters) bool {
	return filters.RazaoSocial != "" || filters.NomeFantasia != ""
}

func fuzzySearchTotal(offset, limit int, fetched int64) int64 {
	if fetched > int64(limit) {
		return int64(offset + limit + 1)
	}
	return int64(offset) + fetched
}

func streamCSV(
	ctx context.Context,
	db *sql.DB,
	w io.Writer,
	query string,
	args []interface{},
	columns []string,
) error {
	header := strings.Join(columns, ";") + "\n"
	if _, err := w.Write([]byte(header)); err != nil {
		return fmt.Errorf("failed to write CSV header: %w", err)
	}

	rows, err := db.QueryContext(ctx, query, args...)
	if err != nil {
		return fmt.Errorf("failed to query export rows: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		values := make([]interface{}, len(columns))
		valuePtrs := make([]interface{}, len(columns))
		for i := range values {
			valuePtrs[i] = &values[i]
		}

		if scanErr := rows.Scan(valuePtrs...); scanErr != nil {
			return fmt.Errorf("failed to scan row: %w", scanErr)
		}

		csvRow := make([]string, len(columns))
		for i, val := range values {
			if val == nil {
				csvRow[i] = ""
				continue
			}
			csvRow[i] = fmt.Sprintf("%v", val)
		}

		rowStr := strings.Join(csvRow, ";") + "\n"
		if _, err := w.Write([]byte(rowStr)); err != nil {
			return fmt.Errorf("failed to write CSV row: %w", err)
		}
	}

	if err := rows.Err(); err != nil {
		return fmt.Errorf("failed iterating export rows: %w", err)
	}

	return nil
}
