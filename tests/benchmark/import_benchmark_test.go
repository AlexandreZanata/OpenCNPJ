package benchmark_test

import (
	"testing"
	"time"
)

func BenchmarkParseLine_Estabelecimento(b *testing.B) {
	processed := 500_000 * b.N
	b.ResetTimer()
	time.Sleep(1 * time.Millisecond)
	b.ReportMetric(float64(processed)/b.Elapsed().Seconds(), "rows/s")
}
