package metrics

import (
	"context"
	"errors"
	"fmt"
	"log"
	"net/http"
	"sync/atomic"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promhttp"
)

type Collector struct {
	dataset    string
	totalRows  atomic.Int64
	totalBytes atomic.Uint64
	totalErrs  atomic.Int64
}

var rowsCounter = prometheus.NewCounterVec(
	prometheus.CounterOpts{
		Name: "import_rows_total",
		Help: "Total imported rows by dataset.",
	},
	[]string{"dataset"},
)

func init() {
	prometheus.MustRegister(rowsCounter)
}

func NewCollector(dataset string) *Collector {
	return &Collector{dataset: dataset}
}

func (c *Collector) AddRows(n int64) {
	c.totalRows.Add(n)
	rowsCounter.WithLabelValues(c.dataset).Add(float64(n))
}

func (c *Collector) AddBytes(n uint64) {
	c.totalBytes.Add(n)
}

func (c *Collector) AddError() {
	c.totalErrs.Add(1)
}

func (c *Collector) StartReporter(ctx context.Context, every time.Duration, logger *log.Logger) {
	ticker := time.NewTicker(every)
	defer ticker.Stop()

	var previousRows int64
	var previousBytes uint64
	previousTick := time.Now()

	for {
		select {
		case <-ctx.Done():
			return
		case now := <-ticker.C:
			totalRows := c.totalRows.Load()
			totalBytes := c.totalBytes.Load()
			totalErrs := c.totalErrs.Load()

			deltaRows := totalRows - previousRows
			deltaBytes := totalBytes - previousBytes
			secs := now.Sub(previousTick).Seconds()
			var rowsPerSec float64
			var mbPerSec float64
			if secs > 0 {
				rowsPerSec = float64(deltaRows) / secs
				mbPerSec = (float64(deltaBytes) / (1024.0 * 1024.0)) / secs
			}

			logger.Printf(
				"[%s] %s | %d rows | %.0f rows/s | %.1f MB/s | %d errors",
				now.Format("2006-01-02 15:04:05"),
				c.dataset,
				totalRows,
				rowsPerSec,
				mbPerSec,
				totalErrs,
			)

			previousRows = totalRows
			previousBytes = totalBytes
			previousTick = now
		}
	}
}

func StartPrometheusServer(ctx context.Context, addr string) error {
	mux := http.NewServeMux()
	mux.Handle("/metrics", promhttp.Handler())
	server := &http.Server{
		Addr:              addr,
		Handler:           mux,
		ReadHeaderTimeout: 5 * time.Second,
	}

	go func() {
		<-ctx.Done()
		shutdownCtx, cancel := context.WithTimeout(context.WithoutCancel(ctx), 5*time.Second)
		defer cancel()
		_ = server.Shutdown(shutdownCtx)
	}()

	if err := server.ListenAndServe(); err != nil && !errors.Is(err, http.ErrServerClosed) {
		return fmt.Errorf("prometheus server error: %w", err)
	}
	return nil
}
