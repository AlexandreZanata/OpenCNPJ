package services

import (
	"context"
	"errors"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

type SearchService struct {
	empresaRepo         *repository.EmpresaRepository
	estabelecimentoRepo *repository.EstabelecimentoRepository
	cache               *CacheService
}

var errEstabelecimentoNotFound = errors.New("estabelecimento not found")

func NewSearchService() *SearchService {
	return &SearchService{
		empresaRepo:         repository.NewEmpresaRepository(),
		estabelecimentoRepo: repository.NewEstabelecimentoRepository(),
		cache:               NewCacheService(),
	}
}

//nolint:gocritic // Keeping value argument to avoid broad API churn now.
func (s *SearchService) SearchEmpresas(
	ctx context.Context,
	filters models.SearchFilters,
) (*models.SearchResponse, error) {
	cacheKey := s.cache.GenerateKey("empresas:search:v4", map[string]interface{}{
		"uuid_id":           filters.UUIDID,
		"cnpj_basico":       filters.CNPJBasico,
		"razao_social":      filters.RazaoSocial,
		"natureza_juridica": filters.NaturezaJuridica,
		"porte_empresa":     filters.PorteEmpresa,
		"capital_min":       filters.CapitalSocialMin,
		"capital_max":       filters.CapitalSocialMax,
		"limit":             filters.Limit,
		"offset":            filters.Offset,
	})

	return GetOrSetJSON(ctx, s.cache, cacheKey, func() (*models.SearchResponse, error) {
		empresas, total, err := s.empresaRepo.SearchEmpresas(ctx, filters)
		if err != nil {
			return nil, err
		}
		basicos := make([]string, 0, len(empresas))
		for _, emp := range empresas {
			basicos = append(basicos, emp.CNPJBasico)
		}
		full, estabs, socios, simples, err := s.loadRelatedByBasicos(ctx, basicos)
		if err != nil {
			return nil, err
		}
		aggregates := repository.BuildEmpresaAggregates(empresas, full, estabs, socios, simples)

		return &models.SearchResponse{
			Data:    aggregates,
			Total:   total,
			Limit:   filters.Limit,
			Offset:  filters.Offset,
			HasMore: filters.Offset+filters.Limit < int(total),
		}, nil
	})
}

//nolint:gocritic // Keeping value argument to avoid broad API churn now.
func (s *SearchService) SearchEstabelecimentos(
	ctx context.Context,
	filters models.SearchFilters,
) (*models.SearchResponse, error) {
	cacheKey := s.cache.GenerateKey("estabelecimentos:search:v4", map[string]interface{}{
		"uuid_id":        filters.UUIDID,
		"cnpj_completo":  filters.CNPJCompleto,
		"cnpj_basico":    filters.CNPJBasico,
		"nome_fantasia":  filters.NomeFantasia,
		"cnae_principal": filters.CNAEPrincipal,
		"uf":             filters.UF,
		"municipio":      filters.Municipio,
		"situacao":       filters.SituacaoCadastral,
		"cep":            filters.CEP,
		"limit":          filters.Limit,
		"offset":         filters.Offset,
	})

	return GetOrSetJSON(ctx, s.cache, cacheKey, func() (*models.SearchResponse, error) {
		estabelecimentos, total, err := s.estabelecimentoRepo.SearchEstabelecimentos(ctx, filters)
		if err != nil {
			return nil, err
		}
		basicos := make([]string, 0, len(estabelecimentos))
		for _, est := range estabelecimentos {
			basicos = append(basicos, est.CNPJBasico)
		}
		full, _, socios, simples, err := s.loadRelatedByBasicos(ctx, basicos)
		if err != nil {
			return nil, err
		}
		results := repository.BuildEstabelecimentoSearchResults(estabelecimentos, full, socios, simples)

		return &models.SearchResponse{
			Data:    results,
			Total:   total,
			Limit:   filters.Limit,
			Offset:  filters.Offset,
			HasMore: filters.Offset+filters.Limit < int(total),
		}, nil
	})
}

func (s *SearchService) GetEstabelecimentoByCNPJ(
	ctx context.Context,
	cnpj string,
) (*models.EstabelecimentoSearchResult, error) {
	cacheKey := "estabelecimento:cnpj:v2:" + cnpj

	result, err := GetOrSetJSON(ctx, s.cache, cacheKey, func() (*models.EstabelecimentoSearchResult, error) {
		est, err := s.estabelecimentoRepo.GetByCNPJCompleto(ctx, cnpj)
		if err != nil {
			return nil, err
		}
		if est == nil {
			return nil, nil
		}
		full, _, socios, simples, err := s.loadRelatedByBasicos(ctx, []string{est.CNPJBasico})
		if err != nil {
			return nil, err
		}
		results := repository.BuildEstabelecimentoSearchResults(
			[]models.EstabelecimentoCompleto{*est},
			full,
			socios,
			simples,
		)
		if len(results) == 0 {
			return nil, nil
		}
		return &results[0], nil
	})
	if err != nil {
		return nil, err
	}
	if result == nil {
		return nil, errEstabelecimentoNotFound
	}

	return result, nil
}
