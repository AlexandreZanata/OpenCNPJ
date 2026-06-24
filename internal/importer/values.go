package importer

import (
	"fmt"
	"strings"

	"busca-cnpj-2026/internal/model"
)

func sanitize(v string) string {
	return strings.ReplaceAll(v, "\x00", "")
}

func nullStr(v string) any {
	v = sanitize(v)
	if v == "" {
		return nil
	}
	return v
}

func dateVal(d *model.Date) any {
	if d == nil {
		return nil
	}
	return d.Time
}

func int16Str(v int16) string {
	return fmt.Sprintf("%d", v)
}

func int16Pad2(v int16) string {
	return fmt.Sprintf("%02d", v)
}

func fkOrNil(ok func(string) bool, v string) any {
	v = sanitize(v)
	if v == "" {
		return nil
	}
	if ok(v) {
		return v
	}
	return nil
}

func cleanRow(row []any) []any {
	for i, v := range row {
		if s, ok := v.(string); ok {
			row[i] = sanitize(s)
		}
	}
	return row
}
