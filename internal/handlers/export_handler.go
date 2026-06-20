package handlers

import (
	"strings"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/services"
)

type ExportHandler struct {
	exportService *services.ExportService
}

func NewExportHandler() *ExportHandler {
	return &ExportHandler{
		exportService: services.NewExportService(),
	}
}

// ExportCSV handles POST /api/v1/export/csv.
func (h *ExportHandler) ExportCSV(c *fiber.Ctx) error {
	var req models.ExportRequest
	if err := c.BodyParser(&req); err != nil {
		return exportBadRequest(c, err)
	}
	if req.Filters.Limit <= 0 {
		req.Filters.Limit = 10000
	}

	c.Set("Content-Type", "text/csv; charset=utf-8")
	c.Set("Content-Disposition", "attachment; filename=export.csv")
	if err := h.exportService.ExportCSV(c.Context(), c.Response().BodyWriter(), req); err != nil {
		return exportFailed(c, err)
	}
	return nil
}

// ExportPhones handles POST /api/v1/export/phones.
func (h *ExportHandler) ExportPhones(c *fiber.Ctx) error {
	var req models.PhoneExportRequest
	if err := c.BodyParser(&req); err != nil {
		return exportBadRequest(c, err)
	}
	if req.Format == "" {
		req.Format = "csv"
	}

	ext := "csv"
	contentType := "text/csv; charset=utf-8"
	if strings.EqualFold(req.Format, "txt") {
		ext = "txt"
		contentType = "text/plain; charset=utf-8"
	}

	c.Set("Content-Type", contentType)
	c.Set("Content-Disposition", "attachment; filename=phones-"+req.Category+"."+ext)
	if err := h.exportService.ExportPhones(c.Context(), c.Response().BodyWriter(), req); err != nil {
		return exportFailed(c, err)
	}
	return nil
}

// ListExportCategories handles GET /api/v1/export/categories.
func (h *ExportHandler) ListExportCategories(c *fiber.Ctx) error {
	return c.JSON(h.exportService.ListExportCategories())
}

func exportBadRequest(c *fiber.Ctx, err error) error {
	return c.Status(fiber.StatusBadRequest).JSON(models.ErrorResponse{
		Error:   "invalid_request",
		Message: err.Error(),
		Code:    fiber.StatusBadRequest,
	})
}

func exportFailed(c *fiber.Ctx, err error) error {
	code := fiber.StatusInternalServerError
	if strings.Contains(err.Error(), "unknown category") ||
		strings.Contains(err.Error(), "filter is required") ||
		strings.Contains(err.Error(), "invalid date") {
		code = fiber.StatusBadRequest
	}
	return c.Status(code).JSON(models.ErrorResponse{
		Error:   "export_failed",
		Message: err.Error(),
		Code:    code,
	})
}
