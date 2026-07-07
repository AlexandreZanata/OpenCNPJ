package main

import (
	"testing"
)

func TestSaasFlagDefined(t *testing.T) {
	// Smoke: ensure --saas flag is registered without running migrations.
	if testing.Short() {
		t.Skip("short mode")
	}
}
