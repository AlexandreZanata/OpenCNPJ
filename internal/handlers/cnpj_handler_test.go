package handlers_test

import (
	"context"
	"io"
	"net/http"
	"net/http/httptest"
	"testing"

	"github.com/gofiber/fiber/v2"

	"busca-cnpj-2026/internal/cnpj"
	"busca-cnpj-2026/internal/handlers"
)

type mockLookuper struct {
	resp *cnpj.PublicResponse
	err  error
}

func (m mockLookuper) Lookup(context.Context, string) (*cnpj.PublicResponse, error) {
	return m.resp, m.err
}

func TestCNPJHandlerInvalid400(t *testing.T) {
	h := handlers.NewCNPJHandler(cnpj.NewLookupService(nil, nil), nil)
	app := fiber.New()
	app.Get("/cnpj/:cnpj", h.Get)

	req := httptest.NewRequest(http.MethodGet, "/cnpj/abc", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != fiber.StatusBadRequest {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	_, _ = io.Copy(io.Discard, resp.Body)
}

func TestCNPJHandlerNotFound404(t *testing.T) {
	h := handlers.NewCNPJHandler(mockLookuper{err: cnpj.ErrNotFound}, nil)
	app := fiber.New()
	app.Get("/cnpj/:cnpj", h.Get)

	req := httptest.NewRequest(http.MethodGet, "/cnpj/00000000000191", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != fiber.StatusNotFound {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestCNPJHandlerSuccess200(t *testing.T) {
	h := handlers.NewCNPJHandler(mockLookuper{resp: &cnpj.PublicResponse{
		CNPJ:        "00000000000191",
		RazaoSocial: "TEST SA",
		Socios:      []cnpj.SocioSummary{},
	}}, nil)
	app := fiber.New()
	app.Get("/cnpj/:cnpj", h.Get)

	req := httptest.NewRequest(http.MethodGet, "/cnpj/00000000000191", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != fiber.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}
