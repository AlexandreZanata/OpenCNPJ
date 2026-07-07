package handlers

import (
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/timeout"

	"busca-cnpj-2026/internal/config"
)

const searchTimeout = 5 * time.Second

// RegisterV1Routes mounts API v1 routes according to SaaS config.
func RegisterV1Routes(app *fiber.App, search *SearchHandler, export *ExportHandler, lookup *LookupHandler, stats *StatsHandler) {
	v1 := app.Group("/api/v1")

	if config.AppConfig.SaaS.Enabled && config.AppConfig.SaaS.PublicAPIOnly {
		v1.Get("/cnpj/:cnpj", search.GetEstabelecimentoByCNPJ)
		return
	}

	v1.Get("/empresas/search", timeout.NewWithContext(search.SearchEmpresas, searchTimeout))
	v1.Get("/estabelecimentos/search", timeout.NewWithContext(search.SearchEstabelecimentos, searchTimeout))
	v1.Get("/estabelecimentos/:cnpj", search.GetEstabelecimentoByCNPJ)
	v1.Get("/cnpj/:cnpj", search.GetEstabelecimentoByCNPJ)

	v1.Post("/export/csv", export.ExportCSV)
	v1.Post("/export/phones", export.ExportPhones)
	v1.Get("/export/categories", export.ListExportCategories)

	v1.Get("/lookup/sectors", lookup.SearchSectors)
	v1.Get("/lookup/cnae", lookup.SearchCNAE)
	v1.Get("/lookup/municipio", lookup.SearchMunicipios)
	v1.Get("/lookup/nome-fantasia", lookup.SearchNomeFantasia)
	v1.Get("/lookup/uf", lookup.SearchUF)

	v1.Get("/stats/cnae", stats.StatsPerCNAE)
	v1.Get("/stats/uf", stats.StatsPerUF)
	v1.Get("/stats/cnae/:cnae/uf", stats.StatsPerCNAEAndUF)
	v1.Get("/analytics/summary", stats.AnalyticsSummary)
}
