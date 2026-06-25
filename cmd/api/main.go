package main

import (
	"context"
	"fmt"
	"log"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/bytedance/sonic"
	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/compress"
	"github.com/gofiber/fiber/v2/middleware/healthcheck"
	"github.com/gofiber/fiber/v2/middleware/timeout"
	"github.com/prometheus/client_golang/prometheus/promhttp"

	"busca-cnpj-2026/internal/config"
	"busca-cnpj-2026/internal/database"
	"busca-cnpj-2026/internal/handlers"
	"busca-cnpj-2026/internal/middleware"
)

// fiberPrometheusAdapter adapts Fiber's context to http.ResponseWriter for Prometheus.
type fiberPrometheusAdapter struct {
	c       *fiber.Ctx
	header  http.Header
	status  int
	written bool
}

func newFiberPrometheusAdapter(c *fiber.Ctx) *fiberPrometheusAdapter {
	return &fiberPrometheusAdapter{
		c:      c,
		header: make(http.Header),
		status: 200,
	}
}

func (a *fiberPrometheusAdapter) Header() http.Header {
	return a.header
}

func (a *fiberPrometheusAdapter) Write(b []byte) (int, error) {
	if !a.written {
		// Set status code
		a.c.Status(a.status)
		// Copy headers
		for k, v := range a.header {
			for _, val := range v {
				a.c.Set(k, val)
			}
		}
		a.written = true
	}
	return a.c.Response().BodyWriter().Write(b)
}

func (a *fiberPrometheusAdapter) WriteHeader(statusCode int) {
	a.status = statusCode
}

func main() {
	// Load configuration
	if err := config.Load(); err != nil {
		log.Fatalf("Failed to load config: %v", err)
	}

	// Initialize database connections
	if err := database.InitPostgres(); err != nil {
		log.Fatalf("Failed to initialize PostgreSQL: %v", err)
	}
	defer func() {
		if err := database.ClosePostgres(); err != nil {
			log.Printf("Warning: failed to close PostgreSQL: %v", err)
		}
	}()

	if err := database.InitRedis(); err != nil {
		log.Printf("Warning: Failed to initialize Redis: %v (continuing without cache)", err)
	} else {
		defer func() {
			if err := database.CloseRedis(); err != nil {
				log.Printf("Warning: failed to close Redis: %v", err)
			}
		}()
	}

	// Initialize ClickHouse (optional)
	if err := database.InitClickHouse(); err != nil {
		log.Printf("Warning: Failed to initialize ClickHouse: %v (continuing without analytics)", err)
	} else {
		defer func() {
			if err := database.CloseClickHouse(); err != nil {
				log.Printf("Warning: failed to close ClickHouse: %v", err)
			}
		}()
	}

	// Create Fiber app
	app := fiber.New(fiber.Config{
		Prefork:         config.AppConfig.Server.Prefork,
		ReadBufferSize:  config.AppConfig.Server.ReadBufferSize,
		WriteBufferSize: config.AppConfig.Server.WriteBufferSize,
		JSONEncoder:     sonic.Marshal,
		JSONDecoder:     sonic.Unmarshal,
		ErrorHandler: func(c *fiber.Ctx, err error) error {
			code := fiber.StatusInternalServerError
			if e, ok := err.(*fiber.Error); ok {
				code = e.Code
			}
			return c.Status(code).JSON(fiber.Map{
				"error":   "internal_error",
				"message": err.Error(),
				"code":    code,
			})
		},
	})

	// Middleware
	app.Use(compress.New(compress.Config{Level: compress.LevelBestSpeed}))
	app.Use(middleware.Logger())
	app.Use(middleware.RequestID())
	app.Use(middleware.Recovery())
	app.Use(middleware.CORS())
	app.Use(middleware.RateLimiter())
	app.Use(middleware.Metrics())
	app.Use(middleware.ResponseCache())

	// Health check
	app.Use(healthcheck.New(healthcheck.Config{
		LivenessProbe: func(_ *fiber.Ctx) bool {
			return true
		},
		ReadinessProbe: func(_ *fiber.Ctx) bool {
			ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
			defer cancel()
			return database.DB.PingContext(ctx) == nil
		},
	}))

	// Prometheus metrics endpoint
	// Fiber uses fasthttp which is incompatible with net/http, so we use an adapter
	app.Get("/metrics", func(c *fiber.Ctx) error {
		adapter := newFiberPrometheusAdapter(c)
		req, err := http.NewRequestWithContext(c.Context(), http.MethodGet, "/metrics", http.NoBody)
		if err != nil {
			return err
		}
		promhttp.Handler().ServeHTTP(adapter, req)
		return nil
	})

	// Profiling endpoints (staging or debug)
	if os.Getenv("ENABLE_PPROF") == "true" || config.AppConfig.Logging.Level == "debug" {
		go servePprof(":6060")
		log.Println("pprof enabled on :6060")
	}

	// Initialize handlers
	searchHandler := handlers.NewSearchHandler()
	exportHandler := handlers.NewExportHandler()
	statsHandler := handlers.NewStatsHandler()

	lookupHandler := handlers.NewLookupHandler()

	// Routes
	v1 := app.Group("/api/v1")

	// Search routes (5s timeout on heavy fuzzy queries)
	const searchTimeout = 5 * time.Second
	v1.Get("/empresas/search", timeout.NewWithContext(searchHandler.SearchEmpresas, searchTimeout))
	v1.Get("/estabelecimentos/search", timeout.NewWithContext(searchHandler.SearchEstabelecimentos, searchTimeout))
	v1.Get("/estabelecimentos/:cnpj", searchHandler.GetEstabelecimentoByCNPJ)

	// Export routes
	v1.Post("/export/csv", exportHandler.ExportCSV)
	v1.Post("/export/phones", exportHandler.ExportPhones)
	v1.Get("/export/categories", exportHandler.ListExportCategories)

	v1.Get("/lookup/sectors", lookupHandler.SearchSectors)
	v1.Get("/lookup/cnae", lookupHandler.SearchCNAE)
	v1.Get("/lookup/municipio", lookupHandler.SearchMunicipios)
	v1.Get("/lookup/nome-fantasia", lookupHandler.SearchNomeFantasia)
	v1.Get("/lookup/uf", lookupHandler.SearchUF)

	// Stats routes
	v1.Get("/stats/cnae", statsHandler.StatsPerCNAE)
	v1.Get("/stats/uf", statsHandler.StatsPerUF)
	v1.Get("/stats/cnae/:cnae/uf", statsHandler.StatsPerCNAEAndUF)
	v1.Get("/analytics/summary", statsHandler.AnalyticsSummary)

	// Root endpoint
	app.Get("/", func(c *fiber.Ctx) error {
		return c.JSON(fiber.Map{
			"name":    "Receita Federal API",
			"version": "1.0.0",
			"status":  "running",
		})
	})

	// Start server in a goroutine
	go func() {
		addr := fmt.Sprintf(":%d", config.AppConfig.Server.Port)
		if err := app.Listen(addr); err != nil {
			log.Fatalf("Failed to start server: %v", err)
		}
	}()

	log.Printf("Server started on port %d", config.AppConfig.Server.Port)

	// Graceful shutdown
	quit := make(chan os.Signal, 1)
	signal.Notify(quit, os.Interrupt, syscall.SIGTERM)
	<-quit

	log.Println("Shutting down server...")

	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()

	if err := app.ShutdownWithContext(ctx); err != nil {
		log.Printf("Server forced to shutdown: %v", err)
	}

	log.Println("Server exited")
}
