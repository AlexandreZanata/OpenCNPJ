package handlers

import (
	"strconv"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/models"
	"busca-cnpj-2026/internal/services"
)

type SearchHandler struct {
	searchService *services.SearchService
}

func NewSearchHandler() *SearchHandler {
	return &SearchHandler{
		searchService: services.NewSearchService(),
	}
}

// SearchEmpresas handles GET /api/v1/empresas/search.
func (h *SearchHandler) SearchEmpresas(c *fiber.Ctx) error {
	if err := validateFuzzySearchTerm("razao_social", c.Query("razao_social")); err != nil {
		return c.Status(fiber.StatusBadRequest).JSON(models.ErrorResponse{
			Error:   "invalid_search_term",
			Message: err.Error(),
			Code:    fiber.StatusBadRequest,
		})
	}

	filters := models.SearchFilters{
		UUIDID:           c.Query("uuid_id"),
		CNPJBasico:       c.Query("cnpj_basico"),
		RazaoSocial:      c.Query("razao_social"),
		NaturezaJuridica: c.Query("natureza_juridica"),
		PorteEmpresa:     c.Query("porte_empresa"),
		Limit:            100,
		Offset:           0,
	}

	// Parse limit
	if limitStr := c.Query("limit"); limitStr != "" {
		if limit, err := strconv.Atoi(limitStr); err == nil && limit > 0 && limit <= 1000 {
			filters.Limit = limit
		}
	}

	// Parse offset
	if offsetStr := c.Query("offset"); offsetStr != "" {
		if offset, err := strconv.Atoi(offsetStr); err == nil && offset >= 0 {
			filters.Offset = offset
		}
	}

	// Parse capital_social filters
	if minStr := c.Query("capital_social_min"); minStr != "" {
		if minValue, err := strconv.ParseFloat(minStr, 64); err == nil {
			filters.CapitalSocialMin = &minValue
		}
	}
	if maxStr := c.Query("capital_social_max"); maxStr != "" {
		if maxValue, err := strconv.ParseFloat(maxStr, 64); err == nil {
			filters.CapitalSocialMax = &maxValue
		}
	}

	result, err := h.searchService.SearchEmpresas(c.Context(), filters)
	if err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "search_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
	}

	return c.JSON(result)
}

// SearchEstabelecimentos handles GET /api/v1/estabelecimentos/search.
func (h *SearchHandler) SearchEstabelecimentos(c *fiber.Ctx) error {
	if err := validateFuzzySearchTerm("nome_fantasia", c.Query("nome_fantasia")); err != nil {
		return c.Status(fiber.StatusBadRequest).JSON(models.ErrorResponse{
			Error:   "invalid_search_term",
			Message: err.Error(),
			Code:    fiber.StatusBadRequest,
		})
	}

	filters := models.SearchFilters{
		UUIDID:            c.Query("uuid_id"),
		CNPJCompleto:      c.Query("cnpj"),
		CNPJBasico:        c.Query("cnpj_basico"),
		NomeFantasia:      c.Query("nome_fantasia"),
		CNAEPrincipal:     c.Query("cnae"),
		UF:                c.Query("uf"),
		Municipio:         c.Query("municipio"),
		SituacaoCadastral: c.Query("situacao"),
		CEP:               c.Query("cep"),
		Limit:             100,
		Offset:            0,
	}

	// Parse limit
	if limitStr := c.Query("limit"); limitStr != "" {
		if limit, err := strconv.Atoi(limitStr); err == nil && limit > 0 && limit <= 1000 {
			filters.Limit = limit
		}
	}

	// Parse offset
	if offsetStr := c.Query("offset"); offsetStr != "" {
		if offset, err := strconv.Atoi(offsetStr); err == nil && offset >= 0 {
			filters.Offset = offset
		}
	}

	result, err := h.searchService.SearchEstabelecimentos(c.Context(), filters)
	if err != nil {
		return c.Status(fiber.StatusInternalServerError).JSON(models.ErrorResponse{
			Error:   "search_failed",
			Message: err.Error(),
			Code:    fiber.StatusInternalServerError,
		})
	}

	return c.JSON(result)
}

// GetEstabelecimentoByCNPJ handles GET /api/v1/estabelecimentos/:cnpj.
func (h *SearchHandler) GetEstabelecimentoByCNPJ(c *fiber.Ctx) error {
	cnpj := c.Params("cnpj")
	if cnpj == "" {
		return c.Status(fiber.StatusBadRequest).JSON(models.ErrorResponse{
			Error:   "invalid_cnpj",
			Message: "CNPJ parameter is required",
			Code:    fiber.StatusBadRequest,
		})
	}

	estabelecimento, err := h.searchService.GetEstabelecimentoByCNPJ(c.Context(), cnpj)
	if err != nil {
		return c.Status(fiber.StatusNotFound).JSON(models.ErrorResponse{
			Error:   "not_found",
			Message: err.Error(),
			Code:    fiber.StatusNotFound,
		})
	}

	return c.JSON(estabelecimento)
}
