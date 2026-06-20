package handlers

import (
	"errors"
	"strconv"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
	"busca-cnpj-2026/internal/services"
)

type StatsHandler struct {
	statsService *services.StatsService
}

func NewStatsHandler() *StatsHandler {
	return &StatsHandler{
		statsService: services.NewStatsService(),
	}
}

func parseStatsLimit(raw string, fallback int) int {
	if raw == "" {
		return fallback
	}
	limit, err := strconv.Atoi(raw)
	if err != nil || limit <= 0 || limit > 1000 {
		return fallback
	}
	return limit
}

// StatsPerCNAE handles GET /api/v1/stats/cnae.
func (h *StatsHandler) StatsPerCNAE(c *fiber.Ctx) error {
	limit := parseStatsLimit(c.Query("limit"), 100)
	stats, err := h.statsService.StatsPerCNAE(c.Context(), limit)
	if err != nil {
		return statsError(c, err)
	}
	return c.JSON(stats)
}

// StatsPerUF handles GET /api/v1/stats/uf.
func (h *StatsHandler) StatsPerUF(c *fiber.Ctx) error {
	stats, err := h.statsService.StatsPerUF(c.Context())
	if err != nil {
		return statsError(c, err)
	}
	return c.JSON(stats)
}

// StatsPerCNAEAndUF handles GET /api/v1/stats/cnae/:cnae/uf.
func (h *StatsHandler) StatsPerCNAEAndUF(c *fiber.Ctx) error {
	cnae := c.Params("cnae")
	if cnae == "" {
		return c.Status(fiber.StatusBadRequest).JSON(models.ErrorResponse{
			Error:   "invalid_cnae",
			Message: "CNAE parameter is required",
			Code:    fiber.StatusBadRequest,
		})
	}

	limit := parseStatsLimit(c.Query("limit"), 100)
	stats, err := h.statsService.StatsPerCNAEAndUF(c.Context(), cnae, limit)
	if err != nil {
		return statsError(c, err)
	}
	return c.JSON(stats)
}

// AnalyticsSummary handles GET /api/v1/analytics/summary.
func (h *StatsHandler) AnalyticsSummary(c *fiber.Ctx) error {
	cnaeLimit := parseStatsLimit(c.Query("cnae_limit"), 15)
	cnaeUFLimit := parseStatsLimit(c.Query("cnae_uf_limit"), 10)

	summary, err := h.statsService.AnalyticsSummary(c.Context(), cnaeLimit, cnaeUFLimit)
	if err != nil {
		return statsError(c, err)
	}
	return c.JSON(summary)
}

func statsError(c *fiber.Ctx, err error) error {
	code := fiber.StatusInternalServerError
	if errors.Is(err, repository.ErrStatsNotReady) {
		code = fiber.StatusServiceUnavailable
	}
	return c.Status(code).JSON(models.ErrorResponse{
		Error:   "stats_failed",
		Message: err.Error(),
		Code:    code,
	})
}
