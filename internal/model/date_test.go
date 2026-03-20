package model_test

import (
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"

	"busca-cnpj-2026/internal/model"
)

func TestParseDate(t *testing.T) {
	got, err := model.ParseDate("20230115")
	require.NoError(t, err)
	require.NotNil(t, got)
	assert.Equal(t, time.Date(2023, 1, 15, 0, 0, 0, 0, time.UTC), got.Time)
}

func TestParseDate_Null(t *testing.T) {
	got, err := model.ParseDate("00000000")
	require.NoError(t, err)
	assert.Nil(t, got)
}
