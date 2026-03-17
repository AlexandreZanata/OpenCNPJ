package middleware

import (
	"strconv"
	"time"

	"github.com/gofiber/fiber/v2"
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
)

var (
	httpRequestsTotal = promauto.NewCounterVec(
		prometheus.CounterOpts{
			Name: "http_requests_total",
			Help: "Total number of HTTP requests",
		},
		[]string{"method", "endpoint", "status"},
	)

	httpRequestDuration = promauto.NewHistogramVec(
		prometheus.HistogramOpts{
			Name:    "http_request_duration_seconds",
			Help:    "HTTP request duration in seconds",
			Buckets: []float64{0.01, 0.05, 0.1, 0.5, 1, 2, 5},
		},
		[]string{"method", "endpoint"},
	)

	queryDuration = promauto.NewHistogramVec(
		prometheus.HistogramOpts{
			Name:    "query_duration_seconds",
			Help:    "Database query duration in seconds",
			Buckets: []float64{0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1, 2},
		},
		[]string{"query_type"},
	)
)

func Metrics() fiber.Handler {
	return func(c *fiber.Ctx) error {
		start := time.Now()

		err := c.Next()

		duration := time.Since(start).Seconds()
		status := strconv.Itoa(c.Response().StatusCode())
		method := c.Method()
		path := c.Path()

		httpRequestsTotal.WithLabelValues(method, path, status).Inc()
		httpRequestDuration.WithLabelValues(method, path).Observe(duration)

		return err
	}
}

func RecordQueryDuration(queryType string, duration time.Duration) {
	queryDuration.WithLabelValues(queryType).Observe(duration.Seconds())
}
