package config

import (
	"testing"

	"github.com/spf13/viper"
)

func TestRateLimitDefaults(t *testing.T) {
	setDefaults()
	if got := viper.GetInt("server.rate_limit_max"); got != 6000 {
		t.Fatalf("rate_limit_max default = %d, want 6000", got)
	}
	if got := viper.GetInt("server.rate_limit_window_seconds"); got != 60 {
		t.Fatalf("rate_limit_window_seconds default = %d, want 60", got)
	}
}
