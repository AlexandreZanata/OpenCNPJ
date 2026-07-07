package repository

import (
	"fmt"
	"strings"
)

type textSearchMode int

const (
	textSearchNone textSearchMode = iota
	textSearchTrigram
	textSearchFTS
)

func detectTextSearchMode(term string) textSearchMode {
	trimmed := strings.TrimSpace(term)
	if trimmed == "" {
		return textSearchNone
	}
	if strings.Contains(trimmed, " ") {
		return textSearchFTS
	}
	return textSearchTrigram
}

func fuzzyRazaoSocialWhere(argPos int) string {
	return fmt.Sprintf(" AND razao_social %%> $%d", argPos)
}

func fuzzyNomeFantasiaWhere(argPos int) string {
	return fmt.Sprintf(" AND e.nome_fantasia %%> $%d", argPos)
}

func ftsRazaoSocialWhere(argPos int) string {
	return fmt.Sprintf(" AND busca @@ plainto_tsquery('portuguese', $%d)", argPos)
}

func ftsNomeFantasiaWhere(argPos int) string {
	return fmt.Sprintf(" AND e.busca @@ plainto_tsquery('portuguese', $%d)", argPos)
}

func fuzzyRazaoSocialOrder(argPos int) string {
	return fmt.Sprintf("word_similarity($%d, razao_social) DESC, cnpj_basico ASC", argPos)
}

func fuzzyNomeFantasiaOrder(argPos int) string {
	return fmt.Sprintf("word_similarity($%d, e.nome_fantasia) DESC, e.id ASC", argPos)
}

func ftsRazaoSocialOrder(argPos int) string {
	return fmt.Sprintf("ts_rank(busca, plainto_tsquery('portuguese', $%d)) DESC, cnpj_basico ASC", argPos)
}

func ftsNomeFantasiaOrder(argPos int) string {
	return fmt.Sprintf("ts_rank(e.busca, plainto_tsquery('portuguese', $%d)) DESC, e.id ASC", argPos)
}

func razaoSocialScoreSelect(argPos int, mode textSearchMode) string {
	switch mode {
	case textSearchNone:
		return ""
	case textSearchTrigram:
		return fmt.Sprintf(", word_similarity($%d, razao_social) AS _search_score", argPos)
	case textSearchFTS:
		return fmt.Sprintf(", ts_rank(busca, plainto_tsquery('portuguese', $%d)) AS _search_score", argPos)
	}
	return ""
}

func nomeFantasiaScoreSelect(argPos int, mode textSearchMode) string {
	switch mode {
	case textSearchNone:
		return ""
	case textSearchTrigram:
		return fmt.Sprintf(", word_similarity($%d, e.nome_fantasia) AS _search_score", argPos)
	case textSearchFTS:
		return fmt.Sprintf(", ts_rank(e.busca, plainto_tsquery('portuguese', $%d)) AS _search_score", argPos)
	}
	return ""
}
