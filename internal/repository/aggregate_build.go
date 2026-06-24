package repository

import "busca-cnpj-2026/internal/models"

func GroupEstabelecimentosByBasico(
	items []models.EstabelecimentoCompleto,
) map[string][]models.EstabelecimentoCompleto {
	out := make(map[string][]models.EstabelecimentoCompleto)
	for _, item := range items {
		out[item.CNPJBasico] = append(out[item.CNPJBasico], item)
	}
	return out
}

func GroupSociosByBasico(items []models.Socio) map[string][]models.Socio {
	out := make(map[string][]models.Socio)
	for _, item := range items {
		out[item.CNPJBasico] = append(out[item.CNPJBasico], item)
	}
	return out
}

func BuildEmpresaAggregates(
	empresas []models.Empresa,
	full map[string]models.EmpresaFull,
	estabs map[string][]models.EstabelecimentoCompleto,
	socios map[string][]models.Socio,
	simples map[string]models.Simples,
) []models.EmpresaAggregate {
	out := make([]models.EmpresaAggregate, 0, len(empresas))
	for _, emp := range empresas {
		fullEmp, ok := full[emp.CNPJBasico]
		if !ok {
			fullEmp = models.EmpresaFull{Empresa: emp}
		}
		agg := models.EmpresaAggregate{
			EmpresaFull:      fullEmp,
			Estabelecimentos: estabs[emp.CNPJBasico],
			Socios:           socios[emp.CNPJBasico],
		}
		if agg.Estabelecimentos == nil {
			agg.Estabelecimentos = []models.EstabelecimentoCompleto{}
		}
		if agg.Socios == nil {
			agg.Socios = []models.Socio{}
		}
		if s, ok := simples[emp.CNPJBasico]; ok {
			agg.Simples = &s
		}
		out = append(out, agg)
	}
	return out
}

func BuildEstabelecimentoSearchResults(
	rows []models.EstabelecimentoCompleto,
	full map[string]models.EmpresaFull,
	socios map[string][]models.Socio,
	simples map[string]models.Simples,
) []models.EstabelecimentoSearchResult {
	out := make([]models.EstabelecimentoSearchResult, 0, len(rows))
	for _, row := range rows {
		emp, ok := full[row.CNPJBasico]
		if !ok {
			emp = models.EmpresaFull{
				Empresa: models.Empresa{
					CNPJBasico:  row.CNPJBasico,
					RazaoSocial: row.RazaoSocial.String,
				},
			}
		}
		item := models.EstabelecimentoSearchResult{
			EstabelecimentoCompleto: row,
			Empresa:                 emp,
			Socios:                  socios[row.CNPJBasico],
		}
		if item.Socios == nil {
			item.Socios = []models.Socio{}
		}
		if s, ok := simples[row.CNPJBasico]; ok {
			item.Simples = &s
		}
		out = append(out, item)
	}
	return out
}

func UniqueCNPJBasicos(basicos []string) []string {
	if len(basicos) == 0 {
		return nil
	}
	seen := make(map[string]struct{}, len(basicos))
	out := make([]string, 0, len(basicos))
	for _, b := range basicos {
		if b == "" {
			continue
		}
		if _, ok := seen[b]; ok {
			continue
		}
		seen[b] = struct{}{}
		out = append(out, b)
	}
	return out
}
