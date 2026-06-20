package services

import (
	"context"
	"fmt"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

type StatsService struct {
	repo  *repository.StatsRepository
	cache *CacheService
}

func NewStatsService() *StatsService {
	return &StatsService{
		repo:  repository.NewStatsRepository(),
		cache: NewCacheService(),
	}
}

func (s *StatsService) StatsPerUF(ctx context.Context) ([]models.StatsResponse, error) {
	return GetOrSetJSON(s.cache, ctx, "stats:uf", func() ([]models.StatsResponse, error) {
		return s.repo.StatsPerUF(ctx)
	})
}

func (s *StatsService) StatsPerCNAE(ctx context.Context, limit int) ([]models.StatsResponse, error) {
	key := fmt.Sprintf("stats:cnae:%d", limit)
	return GetOrSetJSON(s.cache, ctx, key, func() ([]models.StatsResponse, error) {
		return s.repo.StatsPerCNAE(ctx, limit)
	})
}

func (s *StatsService) StatsPerCNAEAndUF(
	ctx context.Context,
	cnae string,
	limit int,
) ([]models.StatsResponse, error) {
	key := fmt.Sprintf("stats:cnae:%s:uf:%d", cnae, limit)
	return GetOrSetJSON(s.cache, ctx, key, func() ([]models.StatsResponse, error) {
		return s.repo.StatsPerCNAEAndUF(ctx, cnae, limit)
	})
}

func (s *StatsService) AnalyticsSummary(
	ctx context.Context,
	cnaeLimit int,
	cnaeUFLimit int,
) (*models.AnalyticsSummaryResponse, error) {
	key := fmt.Sprintf("stats:analytics:%d:%d", cnaeLimit, cnaeUFLimit)
	return GetOrSetJSON(s.cache, ctx, key, func() (*models.AnalyticsSummaryResponse, error) {
		return s.repo.AnalyticsSummary(ctx, cnaeLimit, cnaeUFLimit)
	})
}
