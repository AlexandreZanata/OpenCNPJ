package config

import (
	"testing"

	"github.com/spf13/viper"
)

func TestMeilisearchSelectiveDefault(t *testing.T) {
	setDefaults()
	if !viper.GetBool("meilisearch.selective_active_matriz") {
		t.Fatal("selective_active_matriz default must be true")
	}
}
