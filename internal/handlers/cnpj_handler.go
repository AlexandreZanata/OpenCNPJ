package handlers

import (
	"errors"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/cnpj"
	"busca-cnpj-2026/internal/saas"
	saasmw "busca-cnpj-2026/internal/saas/middleware"
)

// CNPJHandler serves the public SaaS CNPJ lookup route.
type CNPJHandler struct {
	lookup cnpj.Lookuper
	usage  saas.UsageRecorder
}

// NewCNPJHandler wires the public CNPJ lookup handler.
func NewCNPJHandler(lookup cnpj.Lookuper, usage saas.UsageRecorder) *CNPJHandler {
	return &CNPJHandler{lookup: lookup, usage: usage}
}

// Get handles GET /api/v1/cnpj/:cnpj.
func (h *CNPJHandler) Get(c *fiber.Ctx) error {
	raw := c.Params("cnpj")
	resp, err := h.lookup.Lookup(c.Context(), raw)
	if err != nil {
		return mapCNPJError(c, err)
	}
	if h.usage != nil {
		saasmw.RecordCNPJLookup(c, h.usage)
	}
	return c.JSON(resp)
}

func mapCNPJError(c *fiber.Ctx, err error) error {
	switch {
	case errors.Is(err, cnpj.ErrInvalidCNPJ):
		return c.Status(fiber.StatusBadRequest).JSON(fiber.Map{
			"error":   "invalid_cnpj",
			"message": err.Error(),
			"code":    fiber.StatusBadRequest,
		})
	case errors.Is(err, cnpj.ErrNotFound):
		return c.Status(fiber.StatusNotFound).JSON(fiber.Map{
			"error":   "cnpj_not_found",
			"message": err.Error(),
			"code":    fiber.StatusNotFound,
		})
	default:
		return c.Status(fiber.StatusInternalServerError).JSON(fiber.Map{
			"error":   "internal_error",
			"message": err.Error(),
			"code":    fiber.StatusInternalServerError,
		})
	}
}
