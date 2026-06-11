package parser_test

import (
	"testing"

	"github.com/stretchr/testify/require"

	"busca-cnpj-2026/internal/parser"
)

func TestParseSimples(t *testing.T) {
	line := []string{"00000011", "S", "20070701", "00000000", "N", "00000000", "00000000"}
	got, err := parser.ParseSimples(line)
	require.NoError(t, err)
	require.Equal(t, "00000011", got.CNPJBasico)
	require.Equal(t, "S", got.OpcaoSimples)
}
