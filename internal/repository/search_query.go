package repository

import "fmt"

func fuzzyRazaoSocialWhere(argPos int) string {
	return fmt.Sprintf(" AND razao_social %% $%d", argPos)
}

func fuzzyNomeFantasiaWhere(argPos int) string {
	return fmt.Sprintf(" AND e.nome_fantasia %% $%d", argPos)
}

func fuzzyRazaoSocialOrder(argPos int) string {
	return fmt.Sprintf("similarity(razao_social, $%d) DESC", argPos)
}

func fuzzyNomeFantasiaOrder(argPos int) string {
	return fmt.Sprintf("similarity(e.nome_fantasia, $%d) DESC", argPos)
}
