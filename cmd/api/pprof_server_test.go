package main

import (
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestNewPprofMuxServesIndex(t *testing.T) {
	mux := newPprofMux()
	req := httptest.NewRequest(http.MethodGet, "/debug/pprof/", http.NoBody)
	rec := httptest.NewRecorder()

	mux.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Fatalf("status = %d, want %d", rec.Code, http.StatusOK)
	}
}
