package importer

import "strings"

func cleanText(value string) string {
	return strings.ReplaceAll(value, "\x00", "")
}

func nullIfEmpty(value string) any {
	value = cleanText(value)
	if value == "" {
		return nil
	}
	return value
}

func dateValue(d interface{ Value() (any, error) }) (any, error) {
	if d == nil {
		return nil, nil
	}
	return d.Value()
}
