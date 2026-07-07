package handlers

import (
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/timeout"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/saas"
	saasmw "busca-cnpj-2026/internal/saas/middleware"
)

const searchTimeout = 5 * time.Second

// RegisterV1Routes mounts API v1 routes according to SaaS config.
func RegisterV1Routes(
	app *fiber.App,
	search *SearchHandler,
	export *ExportHandler,
	lookup *LookupHandler,
	stats *StatsHandler,
	cnpj *CNPJHandler,
	saasDeps *saas.Deps,
) {
	v1 := app.Group("/api/v1")
	if saasDeps != nil {
		v1.Use(saasmw.APIKey(saasDeps))
	}

	if config.AppConfig.SaaS.Enabled && config.AppConfig.SaaS.PublicAPIOnly {
		if cnpj == nil {
			panic("cnpj handler required when saas.public_api_only is enabled")
		}
		v1.Get("/cnpj/:cnpj", cnpj.Get)
		return
	}

	if config.AppConfig.SaaS.Enabled && cnpj != nil {
		v1.Get("/cnpj/:cnpj", cnpj.Get)
	} else {
		v1.Get("/cnpj/:cnpj", wrapCNPJRoute(saasDeps, search.GetEstabelecimentoByCNPJ))
	}

	v1.Get("/empresas/search", timeout.NewWithContext(search.SearchEmpresas, searchTimeout))
	v1.Get("/estabelecimentos/search", timeout.NewWithContext(search.SearchEstabelecimentos, searchTimeout))
	v1.Get("/estabelecimentos/:cnpj", wrapCNPJRoute(saasDeps, search.GetEstabelecimentoByCNPJ))

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

func wrapCNPJRoute(deps *saas.Deps, handler fiber.Handler) fiber.Handler {
	if deps == nil || deps.Usage == nil {
		return handler
	}
	return wrapCNPJUsage(deps.Usage, handler)
}

func wrapCNPJUsage(usage saas.UsageRecorder, handler fiber.Handler) fiber.Handler {
	return func(c *fiber.Ctx) error {
		if err := handler(c); err != nil {
			return err
		}
		if c.Response().StatusCode() == fiber.StatusOK {
			saasmw.RecordCNPJLookup(c, usage)
		}
		return nil
	}
}
