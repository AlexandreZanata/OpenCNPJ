package services

import (
	"context"
	"log"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/meilisearch"
	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

func newMeiliClient() *meilisearch.Client {
	cfg := config.AppConfig.Meilisearch
	if !cfg.Enabled {
		return nil
	}
	return meilisearch.NewClient(cfg.Host, cfg.Port, cfg.APIKey)
}

func meiliEligibleEmpresa(filters models.SearchFilters) bool {
	if filters.RazaoSocial == "" || filters.Cursor != "" {
		return false
	}
	return filters.UUIDID == "" && filters.CNPJBasico == "" &&
		filters.NaturezaJuridica == "" && filters.PorteEmpresa == "" &&
		filters.CapitalSocialMin == nil && filters.CapitalSocialMax == nil
}

func meiliEligibleEstabelecimento(filters models.SearchFilters) bool {
	if filters.NomeFantasia == "" || filters.Cursor != "" {
		return false
	}
	return filters.UUIDID == "" && filters.CNPJCompleto == "" && filters.CNPJBasico == "" &&
		filters.CNAEPrincipal == "" && filters.UF == "" &&
		filters.Municipio == "" && filters.SituacaoCadastral == "" && filters.CEP == ""
}

func (s *SearchService) searchEmpresasMeili(
	ctx context.Context,
	client *meilisearch.Client,
	filters models.SearchFilters,
) (*models.SearchResponse, bool, error) {
	if client == nil || !meiliEligibleEmpresa(filters) {
		return nil, false, nil
	}
	limit := filters.Limit
	if limit <= 0 {
		limit = 100
	}
	fetchLimit := limit + 1
	hits, err := client.Search(ctx, meilisearch.IndexEmpresas, filters.RazaoSocial, fetchLimit, filters.Offset)
	if err != nil {
		log.Printf("meilisearch empresas fallback: %v", err)
		return nil, false, nil
	}
	hasMore := len(hits) > limit
	if hasMore {
		hits = hits[:limit]
	}
	basicos := make([]string, 0, len(hits))
	for _, hit := range hits {
		basicos = append(basicos, hit.ID)
	}
	empresas, err := s.empresaRepo.ListEmpresasByCNPJBasicos(ctx, basicos)
	if err != nil {
		return nil, false, err
	}
	full, estabs, socios, simples, err := s.loadRelatedByBasicos(ctx, basicos)
	if err != nil {
		return nil, false, err
	}
	aggregates := repository.BuildEmpresaAggregates(empresas, full, estabs, socios, simples)
	meta := repository.PageMeta{HasMore: hasMore}
	return buildSearchResponse(aggregates, meta, limit, filters.Offset), true, nil
}

func (s *SearchService) searchEstabelecimentosMeili(
	ctx context.Context,
	client *meilisearch.Client,
	filters models.SearchFilters,
) (*models.SearchResponse, bool, error) {
	if client == nil || !meiliEligibleEstabelecimento(filters) {
		return nil, false, nil
	}
	limit := filters.Limit
	if limit <= 0 {
		limit = 100
	}
	fetchLimit := limit + 1
	hits, err := client.Search(ctx, meilisearch.IndexEstabelecimentos, filters.NomeFantasia, fetchLimit, filters.Offset)
	if err != nil {
		log.Printf("meilisearch estabelecimentos fallback: %v", err)
		return nil, false, nil
	}
	hasMore := len(hits) > limit
	if hasMore {
		hits = hits[:limit]
	}
	ids := make([]string, 0, len(hits))
	for _, hit := range hits {
		ids = append(ids, hit.ID)
	}
	parsed, err := repository.ParseEstabIDsFromStrings(ids)
	if err != nil {
		return nil, false, err
	}
	estabelecimentos, err := s.estabelecimentoRepo.ListEstabelecimentosByIDs(ctx, parsed)
	if err != nil {
		return nil, false, err
	}
	basicos := make([]string, 0, len(estabelecimentos))
	for i := range estabelecimentos {
		basicos = append(basicos, estabelecimentos[i].CNPJBasico)
	}
	full, _, socios, simples, err := s.loadRelatedByBasicos(ctx, basicos)
	if err != nil {
		return nil, false, err
	}
	results := repository.BuildEstabelecimentoSearchResults(estabelecimentos, full, socios, simples)
	meta := repository.PageMeta{HasMore: hasMore}
	return buildSearchResponse(results, meta, limit, filters.Offset), true, nil
}
