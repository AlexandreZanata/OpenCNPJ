package parser_test

import (
	"context"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"busca-cnpj-2026/internal/parser"
)

func TestParseEstabelecimento(t *testing.T) {
	line := []string{
		"43217850", "0051", "18", "2", "NOME FANTASIA", "08", "20070402", "00", "",
		"0001", "20060101", "0111301", "", "RUA", "DAS FLORES", "123", "", "CENTRO",
		"12345678", "SP", "7107", "11", "999999999", "", "", "", "", "x@x.com", "", "00000000",
	}

	lookups := parser.NewLookupStore()
	lookups.LoadNaturezas(context.Background(), []string{"2062"})
	lookups.LoadPaises(context.Background(), []string{"0001"})
	lookups.LoadCNAEs(context.Background(), []string{"0111301"})
	lookups.LoadMunicipios(context.Background(), []string{"7107"})
	got, err := parser.ParseEstabelecimento(line, lookups)

	require.NoError(t, err)
	assert.Equal(t, "43217850", got.CNPJBasico)
	require.NotNil(t, got.DataSituacao)
	assert.Equal(t, time.Date(2007, 4, 2, 0, 0, 0, 0, time.UTC), got.DataSituacao.Time)
	assert.Nil(t, got.DataSituacaoEspecial)
}

func TestParseEstabelecimento_InvalidColumns(t *testing.T) {
	_, err := parser.ParseEstabelecimento([]string{"1", "2"}, nil)
	require.Error(t, err)
	_, ok := err.(parser.InvalidColumnCountError)
	assert.True(t, ok)
}
