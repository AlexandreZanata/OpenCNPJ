package repository

import (
	"fmt"
	"strings"
	"unicode"
)

const accentFrom = "áàâãäéèêëíìîïóòôõöúùûüç"
const accentTo = "aaaaaeeeeiiiiooooouuuuc"

func splitLookupTerms(query string) []string {
	normalized := foldAccents(query)
	if normalized == "" {
		return nil
	}
	if isDigitsOnly(normalized) {
		return []string{normalized}
	}

	parts := strings.Fields(normalized)
	terms := make([]string, 0, len(parts))
	for _, part := range parts {
		part = strings.TrimSpace(part)
		if len(part) >= 2 {
			terms = append(terms, part)
		}
	}
	if len(terms) == 0 && len(normalized) >= 2 {
		return []string{normalized}
	}
	return terms
}

func foldAccents(value string) string {
	replacer := strings.NewReplacer(
		"á", "a", "à", "a", "â", "a", "ã", "a", "ä", "a",
		"é", "e", "è", "e", "ê", "e", "ë", "e",
		"í", "i", "ì", "i", "î", "i", "ï", "i",
		"ó", "o", "ò", "o", "ô", "o", "õ", "o", "ö", "o",
		"ú", "u", "ù", "u", "û", "u", "ü", "u",
		"ç", "c",
	)
	return replacer.Replace(strings.ToLower(strings.TrimSpace(value)))
}

func accentFoldExpr(column string) string {
	return fmt.Sprintf("translate(lower(%s), '%s', '%s')", column, accentFrom, accentTo)
}

func isDigitsOnly(value string) bool {
	if value == "" {
		return false
	}
	for _, r := range value {
		if !unicode.IsDigit(r) {
			return false
		}
	}
	return true
}

func buildCNAEDescricaoMatch(terms []string, argPos *int, args *[]any) string {
	if len(terms) == 0 {
		return "FALSE"
	}
	parts := make([]string, 0, len(terms))
	foldedColumn := accentFoldExpr("descricao")
	for _, term := range terms {
		parts = append(parts, fmt.Sprintf("%s ILIKE $%d", foldedColumn, *argPos))
		*argPos++
		*args = append(*args, "%"+foldAccents(term)+"%")
	}
	return strings.Join(parts, " AND ")
}

func cnaeLookupMinLen(term string) bool {
	return len(term) >= 2 || isDigitsOnly(term)
}
