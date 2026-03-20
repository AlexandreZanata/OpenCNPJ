package parser_test

import (
	"context"
	"testing"

	"github.com/shopspring/decimal"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"busca-cnpj-2026/internal/parser"
)

func TestParseEmpresa(t *testing.T) {
	lookups := parser.NewLookupStore()
	lookups.LoadNaturezas(context.Background(), []string{"2062"})

	line := []string{"43217850", "EMPRESA XPTO", "2062", "10", "1.000,00", "01", ""}
	got, err := parser.ParseEmpresa(line, lookups)

	require.NoError(t, err)
	assert.Equal(t, "43217850", got.CNPJBasico)
	assert.True(t, got.CapitalSocial.Equal(decimal.RequireFromString("1000.00")))
}

func TestParseEmpresa_InvalidColumns(t *testing.T) {
	_, err := parser.ParseEmpresa([]string{"43217850"}, nil)
	require.Error(t, err)
}
