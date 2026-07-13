package main

import "testing"

func TestEnvOr(t *testing.T) {
	t.Setenv("API_CLIENT_NAME", "Acme")
	if got := envOr("API_CLIENT_NAME", "x"); got != "Acme" {
		t.Fatalf("got %q", got)
	}
}
