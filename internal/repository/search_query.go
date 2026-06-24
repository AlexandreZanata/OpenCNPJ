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
	return fmt.Sprintf(" AND razao_social %% $%d", argPos)
}

func fuzzyNomeFantasiaWhere(argPos int) string {
	return fmt.Sprintf(" AND e.nome_fantasia %% $%d", argPos)
}

func ftsRazaoSocialWhere(argPos int) string {
	return fmt.Sprintf(" AND busca @@ plainto_tsquery('portuguese', $%d)", argPos)
}

func ftsNomeFantasiaWhere(argPos int) string {
	return fmt.Sprintf(" AND e.busca @@ plainto_tsquery('portuguese', $%d)", argPos)
}

func fuzzyRazaoSocialOrder(argPos int) string {
	return fmt.Sprintf("similarity(razao_social, $%d) DESC, cnpj_basico ASC", argPos)
}

func fuzzyNomeFantasiaOrder(argPos int) string {
	return fmt.Sprintf("similarity(e.nome_fantasia, $%d) DESC, e.id ASC", argPos)
}

func ftsRazaoSocialOrder(argPos int) string {
	return fmt.Sprintf("ts_rank(busca, plainto_tsquery('portuguese', $%d)) DESC, cnpj_basico ASC", argPos)
}

func ftsNomeFantasiaOrder(argPos int) string {
	return fmt.Sprintf("ts_rank(e.busca, plainto_tsquery('portuguese', $%d)) DESC, e.id ASC", argPos)
}

func razaoSocialScoreSelect(argPos int, mode textSearchMode) string {
	switch mode {
	case textSearchTrigram:
		return fmt.Sprintf(", similarity(razao_social, $%d) AS _search_score", argPos)
	case textSearchFTS:
		return fmt.Sprintf(", ts_rank(busca, plainto_tsquery('portuguese', $%d)) AS _search_score", argPos)
	default:
		return ""
	}
}

func nomeFantasiaScoreSelect(argPos int, mode textSearchMode) string {
	switch mode {
	case textSearchTrigram:
		return fmt.Sprintf(", similarity(e.nome_fantasia, $%d) AS _search_score", argPos)
	case textSearchFTS:
		return fmt.Sprintf(", ts_rank(e.busca, plainto_tsquery('portuguese', $%d)) AS _search_score", argPos)
	default:
		return ""
	}
}
