package parser_test

import (
	"testing"

	"github.com/stretchr/testify/require"

	"busca-cnpj-2026/internal/parser"
)

func TestParseSocio(t *testing.T) {
	line := []string{
		"41481283", "2", "LEONARDO FISTAROL", "***261720**", "49",
		"20210407", "", "***000000**", "", "00", "3",
	}
	got, err := parser.ParseSocio(line, nil)
	require.NoError(t, err)
	require.Equal(t, "41481283", got.CNPJBasico)
	require.NotNil(t, got.DataEntradaSociedade)
}
