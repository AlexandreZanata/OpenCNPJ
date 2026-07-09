package main

import "testing"

func TestEnvOr(t *testing.T) {
	t.Setenv("ADMIN_EMAIL", "a@b.com")
	if got := envOr("ADMIN_EMAIL", "x"); got != "a@b.com" {
		t.Fatalf("got %q", got)
	}
	if got := envOr("MISSING", "fallback"); got != "fallback" {
		t.Fatalf("got %q", got)
	}
}
