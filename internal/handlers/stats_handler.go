package handlers

import (
	"strconv"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/repository"
)

type StatsHandler struct {
	statsRepo *repository.StatsRepository
}

func NewStatsHandler() *StatsHandler {
	return &StatsHandler{
		statsRepo: repository.NewStatsRepository(),
	}
}

// StatsPerCNAE handles GET /api/v1/stats/cnae.
func (h *StatsHandler) StatsPerCNAE(c *fiber.Ctx) error {
	limit := 100
	if limitStr := c.Query("limit"); limitStr != "" {
		if l, err := strconv.Atoi(limitStr); err == nil && l > 0 && l <= 1000 {
			limit = l
		}
	}

	stats, err := h.statsRepo.StatsPerCNAE(c.Context(), limit)
	if err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "stats_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
	}

	return c.JSON(stats)
}

// StatsPerUF handles GET /api/v1/stats/uf.
func (h *StatsHandler) StatsPerUF(c *fiber.Ctx) error {
	stats, err := h.statsRepo.StatsPerUF(c.Context())
	if err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "stats_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
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

	limit := 100
	if limitStr := c.Query("limit"); limitStr != "" {
		if l, err := strconv.Atoi(limitStr); err == nil && l > 0 && l <= 1000 {
			limit = l
		}
	}

	stats, err := h.statsRepo.StatsPerCNAEAndUF(c.Context(), cnae, limit)
	if err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "stats_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
	}

	return c.JSON(stats)
}
