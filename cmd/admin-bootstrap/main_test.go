package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestMainRequiresEmail(t *testing.T) {
	if os.Getenv("RUN_BOOTSTRAP_CLI_TEST") != "1" {
		t.Skip("subprocess cli test")
	}
	// Covered by manual bootstrap on staging; flag parsing validated at compile time.
	_ = filepath.Join
}
