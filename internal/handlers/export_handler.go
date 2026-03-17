package handlers

import (
	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/services"

	"github.com/gofiber/fiber/v2"
)

type ExportHandler struct {
	exportService *services.ExportService
}

func NewExportHandler() *ExportHandler {
	return &ExportHandler{
		exportService: services.NewExportService(),
	}
}

// ExportCSV handles POST /api/v1/export/csv
func (h *ExportHandler) ExportCSV(c *fiber.Ctx) error {
	var req models.ExportRequest
	if err := c.BodyParser(&req); err != nil {
		return c.Status(fiber.StatusBadRequest).JSON(models.ErrorResponse{
			Error:   "invalid_request",
			Message: err.Error(),
			Code:    fiber.StatusBadRequest,
		})
	}

	// Set default limit if not provided
	if req.Filters.Limit <= 0 {
		req.Filters.Limit = 10000
	}

	// Set content type and headers
	c.Set("Content-Type", "text/csv; charset=utf-8")
	c.Set("Content-Disposition", "attachment; filename=export.csv")

	// Stream CSV directly to response
	if err := h.exportService.ExportCSV(c.Context(), c.Response().BodyWriter(), req); err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "export_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
	}

	return nil
}
