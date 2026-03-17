package services

import (
	"context"
	"fmt"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

type SearchService struct {
	empresaRepo        *repository.EmpresaRepository
	estabelecimentoRepo *repository.EstabelecimentoRepository
	cache              *CacheService
}

func NewSearchService() *SearchService {
	return &SearchService{
		empresaRepo:        repository.NewEmpresaRepository(),
		estabelecimentoRepo: repository.NewEstabelecimentoRepository(),
		cache:              NewCacheService(),
	}
}

func (s *SearchService) SearchEmpresas(ctx context.Context, filters models.SearchFilters) (*models.SearchResponse, error) {
	cacheKey := s.cache.GenerateKey("empresas:search", map[string]interface{}{
		"cnpj_basico":        filters.CNPJBasico,
		"razao_social":       filters.RazaoSocial,
		"natureza_juridica":  filters.NaturezaJuridica,
		"porte_empresa":      filters.PorteEmpresa,
		"capital_min":       filters.CapitalSocialMin,
		"capital_max":       filters.CapitalSocialMax,
		"limit":              filters.Limit,
		"offset":             filters.Offset,
	})

	result, err := s.cache.GetOrSet(ctx, cacheKey, func() (interface{}, error) {
		empresas, total, err := s.empresaRepo.SearchEmpresas(ctx, filters)
		if err != nil {
			return nil, err
		}

		return &models.SearchResponse{
			Data:    empresas,
			Total:   total,
			Limit:   filters.Limit,
			Offset:  filters.Offset,
			HasMore: filters.Offset+filters.Limit < int(total),
		}, nil
	})

	if err != nil {
		return nil, err
	}

	return result.(*models.SearchResponse), nil
}

func (s *SearchService) SearchEstabelecimentos(ctx context.Context, filters models.SearchFilters) (*models.SearchResponse, error) {
	cacheKey := s.cache.GenerateKey("estabelecimentos:search", map[string]interface{}{
		"cnpj_completo":      filters.CNPJCompleto,
		"cnpj_basico":        filters.CNPJBasico,
		"nome_fantasia":      filters.NomeFantasia,
		"cnae_principal":    filters.CNAEPrincipal,
		"uf":                 filters.UF,
		"municipio":          filters.Municipio,
		"situacao":           filters.SituacaoCadastral,
		"cep":                filters.CEP,
		"limit":              filters.Limit,
		"offset":             filters.Offset,
	})

	result, err := s.cache.GetOrSet(ctx, cacheKey, func() (interface{}, error) {
		estabelecimentos, total, err := s.estabelecimentoRepo.SearchEstabelecimentos(ctx, filters)
		if err != nil {
			return nil, err
		}

		return &models.SearchResponse{
			Data:    estabelecimentos,
			Total:   total,
			Limit:   filters.Limit,
			Offset:  filters.Offset,
			HasMore: filters.Offset+filters.Limit < int(total),
		}, nil
	})

	if err != nil {
		return nil, err
	}

	return result.(*models.SearchResponse), nil
}

func (s *SearchService) GetEstabelecimentoByCNPJ(ctx context.Context, cnpj string) (*models.EstabelecimentoCompleto, error) {
	cacheKey := fmt.Sprintf("estabelecimento:cnpj:%s", cnpj)

	result, err := s.cache.GetOrSet(ctx, cacheKey, func() (interface{}, error) {
		return s.estabelecimentoRepo.GetByCNPJCompleto(ctx, cnpj)
	})

	if err != nil {
		return nil, err
	}

	if result == nil {
		return nil, fmt.Errorf("estabelecimento not found")
	}

	return result.(*models.EstabelecimentoCompleto), nil
}
