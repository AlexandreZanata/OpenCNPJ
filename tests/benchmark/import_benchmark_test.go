package benchmark_test

import (
	"strings"
	"testing"

	"busca-cnpj-2026/internal/parser"
)

func BenchmarkParseLine_Estabelecimento(b *testing.B) {
	line := []string{
		"12345678", "0001", "95", "1", "NOME FANTASIA", "2", "20240101", "00", "", "105", "20200101",
		"6201500", "", "RUA", "ALFA", "100", "", "CENTRO", "01001000", "SP", "7107",
		"11", "99999999", "", "", "", "", "email@example.com", "", "",
	}
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := parser.ParseEstabelecimento(line, nil); err != nil {
			b.Fatalf("unexpected parse error: %v", err)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "rows/s")
}

func BenchmarkCSVDecoderLatin1(b *testing.B) {
	row := "\"12345678\";\"RAZAO SOCIAL LTDA\";\"2062\";\"49\";\"1000,00\";\"01\";\"\"\n"
	payload := strings.Repeat(row, 30_000)

	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		reader := parser.NewCSVReader(strings.NewReader(payload))
		for {
			if _, err := reader.Read(); err != nil {
				break
			}
		}
	}
}
