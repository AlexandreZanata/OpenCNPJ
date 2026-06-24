package services

import (
	"context"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

func (s *SearchService) loadRelatedByBasicos(
	ctx context.Context,
	basicos []string,
) (
	map[string]models.EmpresaFull,
	map[string][]models.EstabelecimentoCompleto,
	map[string][]models.Socio,
	map[string]models.Simples,
	error,
) {
	unique := repository.UniqueCNPJBasicos(basicos)
	full, err := s.empresaRepo.ListEmpresasFullByBasicos(ctx, unique)
	if err != nil {
		return nil, nil, nil, nil, err
	}
	estabs, err := s.estabelecimentoRepo.ListByCNPJBasicos(ctx, unique)
	if err != nil {
		return nil, nil, nil, nil, err
	}
	sociosList, err := s.empresaRepo.ListSociosByCNPJBasicos(ctx, unique)
	if err != nil {
		return nil, nil, nil, nil, err
	}
	simples, err := s.empresaRepo.ListSimplesByCNPJBasicos(ctx, unique)
	if err != nil {
		return nil, nil, nil, nil, err
	}
	return full,
		repository.GroupEstabelecimentosByBasico(estabs),
		repository.GroupSociosByBasico(sociosList),
		simples,
		nil
}
