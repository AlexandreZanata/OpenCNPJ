package handlers

import (
	"context"
	"strconv"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/services"
)

type LookupHandler struct {
	lookupService *services.LookupService
}

func NewLookupHandler() *LookupHandler {
	return &LookupHandler{lookupService: services.NewLookupService()}
}

func (h *LookupHandler) SearchSectors(c *fiber.Ctx) error {
	return h.lookupJSON(c, func(ctx context.Context) ([]models.LookupItem, error) {
		return h.lookupService.SearchSectors(ctx, c.Query("q"), parseLookupLimit(c))
	})
}

func (h *LookupHandler) SearchCNAE(c *fiber.Ctx) error {
	return h.lookupJSON(c, func(ctx context.Context) ([]models.LookupItem, error) {
		return h.lookupService.SearchCNAE(ctx, c.Query("q"), parseLookupLimit(c))
	})
}

func (h *LookupHandler) SearchMunicipios(c *fiber.Ctx) error {
	return h.lookupJSON(c, func(ctx context.Context) ([]models.LookupItem, error) {
		return h.lookupService.SearchMunicipios(ctx, c.Query("q"), c.Query("uf"), parseLookupLimit(c))
	})
}

func (h *LookupHandler) SearchNomeFantasia(c *fiber.Ctx) error {
	return h.lookupJSON(c, func(ctx context.Context) ([]models.LookupItem, error) {
		return h.lookupService.SearchNomeFantasia(ctx, c.Query("q"), c.Query("uf"), parseLookupLimit(c))
	})
}

func (h *LookupHandler) SearchUF(c *fiber.Ctx) error {
	return c.JSON(h.lookupService.SearchUF(c.Query("q")))
}

func (h *LookupHandler) lookupJSON(
	c *fiber.Ctx,
	fn func(context.Context) ([]models.LookupItem, error),
) error {
	items, err := fn(c.Context())
	if err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "lookup_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
	}
	if items == nil {
		items = []models.LookupItem{}
	}
	return c.JSON(items)
}

func parseLookupLimit(c *fiber.Ctx) int {
	limit := 15
	if raw := c.Query("limit"); raw != "" {
		if parsed, err := strconv.Atoi(raw); err == nil && parsed > 0 && parsed <= 100 {
			limit = parsed
		}
	}
	return limit
}
