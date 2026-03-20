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
	dataset   string
	startedAt time.Time
	totalRows atomic.Int64
	totalMB   atomic.Uint64
	totalErrs atomic.Int64
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
	return &Collector{
		dataset:   dataset,
		startedAt: time.Now().UTC(),
	}
}

func (c *Collector) AddRows(n int64) {
	c.totalRows.Add(n)
	rowsCounter.WithLabelValues(c.dataset).Add(float64(n))
}

func (c *Collector) AddBytes(n uint64) {
	c.totalMB.Add(n)
}

func (c *Collector) AddError() {
	c.totalErrs.Add(1)
}

func (c *Collector) StartReporter(ctx context.Context, every time.Duration, logger *log.Logger) {
	ticker := time.NewTicker(every)
	defer ticker.Stop()

	var previousRows int64
	previousTick := time.Now()

	for {
		select {
		case <-ctx.Done():
			return
		case now := <-ticker.C:
			totalRows := c.totalRows.Load()
			totalBytes := c.totalMB.Load()
			totalErrs := c.totalErrs.Load()

			deltaRows := totalRows - previousRows
			secs := now.Sub(previousTick).Seconds()
			var rowsPerSec float64
			if secs > 0 {
				rowsPerSec = float64(deltaRows) / secs
			}

			totalSecs := now.Sub(c.startedAt).Seconds()
			var mbPerSec float64
			if totalSecs > 0 {
				mbPerSec = (float64(totalBytes) / (1024.0 * 1024.0)) / totalSecs
			}

			logger.Printf(
				"[%s] %s | %d rows | %.0f rows/s | %.1f MB/s | %d erros",
				now.Format("2006-01-02 15:04:05"),
				c.dataset,
				totalRows,
				rowsPerSec,
				mbPerSec,
				totalErrs,
			)

			previousRows = totalRows
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
