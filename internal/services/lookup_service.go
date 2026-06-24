package services

import (
	"context"
	"fmt"
	"strings"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

var brazilianUFs = []models.LookupItem{
	{Type: "uf", Code: "AC", Label: "AC — Acre"},
	{Type: "uf", Code: "AL", Label: "AL — Alagoas"},
	{Type: "uf", Code: "AP", Label: "AP — Amapá"},
	{Type: "uf", Code: "AM", Label: "AM — Amazonas"},
	{Type: "uf", Code: "BA", Label: "BA — Bahia"},
	{Type: "uf", Code: "CE", Label: "CE — Ceará"},
	{Type: "uf", Code: "DF", Label: "DF — Distrito Federal"},
	{Type: "uf", Code: "ES", Label: "ES — Espírito Santo"},
	{Type: "uf", Code: "GO", Label: "GO — Goiás"},
	{Type: "uf", Code: "MA", Label: "MA — Maranhão"},
	{Type: "uf", Code: "MT", Label: "MT — Mato Grosso"},
	{Type: "uf", Code: "MS", Label: "MS — Mato Grosso do Sul"},
	{Type: "uf", Code: "MG", Label: "MG — Minas Gerais"},
	{Type: "uf", Code: "PA", Label: "PA — Pará"},
	{Type: "uf", Code: "PB", Label: "PB — Paraíba"},
	{Type: "uf", Code: "PR", Label: "PR — Paraná"},
	{Type: "uf", Code: "PE", Label: "PE — Pernambuco"},
	{Type: "uf", Code: "PI", Label: "PI — Piauí"},
	{Type: "uf", Code: "RJ", Label: "RJ — Rio de Janeiro"},
	{Type: "uf", Code: "RN", Label: "RN — Rio Grande do Norte"},
	{Type: "uf", Code: "RS", Label: "RS — Rio Grande do Sul"},
	{Type: "uf", Code: "RO", Label: "RO — Rondônia"},
	{Type: "uf", Code: "RR", Label: "RR — Roraima"},
	{Type: "uf", Code: "SC", Label: "SC — Santa Catarina"},
	{Type: "uf", Code: "SP", Label: "SP — São Paulo"},
	{Type: "uf", Code: "SE", Label: "SE — Sergipe"},
	{Type: "uf", Code: "TO", Label: "TO — Tocantins"},
}

type LookupService struct {
	repo  *repository.LookupRepository
	cache *CacheService
}

func NewLookupService() *LookupService {
	return &LookupService{
		repo:  repository.NewLookupRepository(),
		cache: NewCacheService(),
	}
}

func (s *LookupService) SearchSectors(ctx context.Context, query string, limit int) ([]models.LookupItem, error) {
	key := fmt.Sprintf("lookup:sectors:%s:%d", query, limit)
	return GetOrSetJSON(ctx, s.cache, key, func() ([]models.LookupItem, error) {
		return s.repo.SearchSectors(ctx, query, limit)
	})
}

func (s *LookupService) SearchCNAE(ctx context.Context, query string, limit int) ([]models.LookupItem, error) {
	key := fmt.Sprintf("lookup:cnae:%s:%d", query, limit)
	return GetOrSetJSON(ctx, s.cache, key, func() ([]models.LookupItem, error) {
		return s.repo.SearchCNAE(ctx, query, limit)
	})
}

func (s *LookupService) SearchMunicipios(
	ctx context.Context,
	query, uf string,
	limit int,
) ([]models.LookupItem, error) {
	key := fmt.Sprintf("lookup:municipio:%s:%s:%d", uf, query, limit)
	return GetOrSetJSON(ctx, s.cache, key, func() ([]models.LookupItem, error) {
		return s.repo.SearchMunicipios(ctx, query, uf, limit)
	})
}

func (s *LookupService) SearchNomeFantasia(
	ctx context.Context,
	query, uf string,
	limit int,
) ([]models.LookupItem, error) {
	if len(strings.TrimSpace(query)) < 3 {
		return nil, nil
	}
	key := fmt.Sprintf("lookup:nome:%s:%s:%d", uf, query, limit)
	return GetOrSetJSON(ctx, s.cache, key, func() ([]models.LookupItem, error) {
		return s.repo.SearchNomeFantasia(ctx, query, uf, limit)
	})
}

func (s *LookupService) SearchUF(query string) []models.LookupItem {
	q := strings.ToLower(strings.TrimSpace(query))
	if q == "" {
		return brazilianUFs
	}
	out := make([]models.LookupItem, 0, 5)
	for _, item := range brazilianUFs {
		if strings.Contains(strings.ToLower(item.Code), q) ||
			strings.Contains(strings.ToLower(item.Label), q) {
			out = append(out, item)
		}
	}
	return out
}
