package meilisearch

import (
	"context"
	"encoding/json"
	"net/http"
	"net/http/httptest"
	"testing"
)

func TestClientSearch(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path != "/indexes/empresas/search" {
			t.Fatalf("path = %s", r.URL.Path)
		}
		_ = json.NewEncoder(w).Encode(map[string]interface{}{
			"hits": []map[string]interface{}{{"id": "33000167"}},
		})
	}))
	defer srv.Close()

	c := &Client{
		baseURL:    srv.URL,
		httpClient: srv.Client(),
	}
	hits, err := c.Search(context.Background(), IndexEmpresas, "PETROBRAS", 10, 0)
	if err != nil {
		t.Fatalf("search: %v", err)
	}
	if len(hits) != 1 || hits[0].ID != "33000167" {
		t.Fatalf("hits = %#v", hits)
	}
}

func TestClientAddDocumentsEmpty(t *testing.T) {
	c := NewClient("localhost", 7700, "key")
	if err := c.AddDocuments(context.Background(), IndexEmpresas, nil); err != nil {
		t.Fatalf("empty add: %v", err)
	}
}

func TestClientHealth(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	c := &Client{baseURL: srv.URL, httpClient: srv.Client()}
	if err := c.Health(context.Background()); err != nil {
		t.Fatalf("health: %v", err)
	}
}
