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
