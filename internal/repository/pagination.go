package repository

import (
	"fmt"
	"strconv"
	"strings"
)

func useKeysetPagination(cursor string, offset int) bool {
	return cursor != "" && offset == 0
}

func parseSearchCursor(cursor string) (map[string]string, error) {
	parts := make(map[string]string)
	for _, segment := range strings.Split(cursor, "|") {
		if segment == "" {
			continue
		}
		kv := strings.SplitN(segment, ":", 2)
		if len(kv) != 2 || kv[0] == "" || kv[1] == "" {
			return nil, fmt.Errorf("invalid cursor segment %q", segment)
		}
		parts[kv[0]] = kv[1]
	}
	if len(parts) == 0 {
		return nil, fmt.Errorf("empty cursor")
	}
	return parts, nil
}

func buildCNPJCursor(cnpjBasico string) string {
	return "cnpj:" + cnpjBasico
}

func buildIDCursor(id int64) string {
	return "id:" + strconv.FormatInt(id, 10)
}

func buildScoreCNPJCursor(score float64, cnpjBasico string) string {
	return fmt.Sprintf("score:%.8f|cnpj:%s", score, cnpjBasico)
}

func buildScoreIDCursor(score float64, id int64) string {
	return fmt.Sprintf("score:%.8f|id:%d", score, id)
}

func empresaKeysetClause(
	cursor string,
	textPos int,
	useTextScore bool,
	argPos *int,
	args *[]interface{},
) (string, error) {
	parts, err := parseSearchCursor(cursor)
	if err != nil {
		return "", err
	}

	if useTextScore && textPos > 0 {
		scoreStr, ok := parts["score"]
		if !ok {
			return "", fmt.Errorf("cursor missing score")
		}
		cnpj, ok := parts["cnpj"]
		if !ok {
			return "", fmt.Errorf("cursor missing cnpj")
		}
		score, err := strconv.ParseFloat(scoreStr, 64)
		if err != nil {
			return "", fmt.Errorf("invalid cursor score: %w", err)
		}
		clause := fmt.Sprintf(
			" AND (similarity(razao_social, $%d), cnpj_basico) < ($%d, $%d)",
			textPos, *argPos, *argPos+1,
		)
		*args = append(*args, score, cnpj)
		*argPos += 2
		return clause, nil
	}

	cnpj, ok := parts["cnpj"]
	if !ok {
		return "", fmt.Errorf("cursor missing cnpj")
	}
	clause := fmt.Sprintf(" AND cnpj_basico > $%d", *argPos)
	*args = append(*args, cnpj)
	*argPos++
	return clause, nil
}

func estabelecimentoKeysetClause(
	cursor string,
	textPos int,
	useTextScore bool,
	argPos *int,
	args *[]interface{},
) (string, error) {
	parts, err := parseSearchCursor(cursor)
	if err != nil {
		return "", err
	}

	if useTextScore && textPos > 0 {
		scoreStr, ok := parts["score"]
		if !ok {
			return "", fmt.Errorf("cursor missing score")
		}
		idStr, ok := parts["id"]
		if !ok {
			return "", fmt.Errorf("cursor missing id")
		}
		score, err := strconv.ParseFloat(scoreStr, 64)
		if err != nil {
			return "", fmt.Errorf("invalid cursor score: %w", err)
		}
		id, err := strconv.ParseInt(idStr, 10, 64)
		if err != nil {
			return "", fmt.Errorf("invalid cursor id: %w", err)
		}
		clause := fmt.Sprintf(
			" AND (similarity(e.nome_fantasia, $%d), e.id) < ($%d, $%d)",
			textPos, *argPos, *argPos+1,
		)
		*args = append(*args, score, id)
		*argPos += 2
		return clause, nil
	}

	idStr, ok := parts["id"]
	if !ok {
		return "", fmt.Errorf("cursor missing id")
	}
	id, err := strconv.ParseInt(idStr, 10, 64)
	if err != nil {
		return "", fmt.Errorf("invalid cursor id: %w", err)
	}
	clause := fmt.Sprintf(" AND e.id > $%d", *argPos)
	*args = append(*args, id)
	*argPos++
	return clause, nil
}

func empresaFTSKeysetClause(cursor string, textPos int, argPos *int, args *[]interface{}) (string, error) {
	parts, err := parseSearchCursor(cursor)
	if err != nil {
		return "", err
	}
	scoreStr, ok := parts["score"]
	if !ok {
		return "", fmt.Errorf("cursor missing score")
	}
	cnpj, ok := parts["cnpj"]
	if !ok {
		return "", fmt.Errorf("cursor missing cnpj")
	}
	score, err := strconv.ParseFloat(scoreStr, 64)
	if err != nil {
		return "", fmt.Errorf("invalid cursor score: %w", err)
	}
	clause := fmt.Sprintf(
		" AND (ts_rank(busca, plainto_tsquery('portuguese', $%d)), cnpj_basico) < ($%d, $%d)",
		textPos, *argPos, *argPos+1,
	)
	*args = append(*args, score, cnpj)
	*argPos += 2
	return clause, nil
}

func estabelecimentoFTSKeysetClause(cursor string, textPos int, argPos *int, args *[]interface{}) (string, error) {
	parts, err := parseSearchCursor(cursor)
	if err != nil {
		return "", err
	}
	scoreStr, ok := parts["score"]
	if !ok {
		return "", fmt.Errorf("cursor missing score")
	}
	idStr, ok := parts["id"]
	if !ok {
		return "", fmt.Errorf("cursor missing id")
	}
	score, err := strconv.ParseFloat(scoreStr, 64)
	if err != nil {
		return "", fmt.Errorf("invalid cursor score: %w", err)
	}
	id, err := strconv.ParseInt(idStr, 10, 64)
	if err != nil {
		return "", fmt.Errorf("invalid cursor id: %w", err)
	}
	clause := fmt.Sprintf(
		" AND (ts_rank(e.busca, plainto_tsquery('portuguese', $%d)), e.id) < ($%d, $%d)",
		textPos, *argPos, *argPos+1,
	)
	*args = append(*args, score, id)
	*argPos += 2
	return clause, nil
}
