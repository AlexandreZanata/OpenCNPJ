package handlers_test

import (
	"context"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/gofiber/fiber/v2"
	"github.com/google/uuid"

	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/handlers"
	"busca-cnpj-2026/internal/adminauth/usecase"
)

func TestPostLoginMFARequired(t *testing.T) {
	h := handlers.NewAuthHandler(
		func(context.Context, usecase.LoginInput) (usecase.LoginMFARequired, error) {
			return usecase.LoginMFARequired{ChallengeID: uuid.New(), ExpiresInSeconds: 300}, nil
		},
		nil, nil, "opencnpj_admin_refresh",
	)
	app := fiber.New()
	app.Post("/login", h.PostLogin)
	req := httptest.NewRequest(http.MethodPost, "/login", strings.NewReader(`{"email":"a@b.c","password":"x"}`))
	req.Header.Set("Content-Type", "application/json")
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status=%d", resp.StatusCode)
	}
}

func TestPostLoginInvalidCredentials401(t *testing.T) {
	h := handlers.NewAuthHandler(
		func(context.Context, usecase.LoginInput) (usecase.LoginMFARequired, error) {
			return usecase.LoginMFARequired{}, autherr.ErrInvalidCredentials
		},
		nil, nil, "opencnpj_admin_refresh",
	)
	app := fiber.New()
	app.Post("/login", h.PostLogin)
	req := httptest.NewRequest(http.MethodPost, "/login", strings.NewReader(`{"email":"a@b.c","password":"bad"}`))
	req.Header.Set("Content-Type", "application/json")
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != http.StatusUnauthorized {
		t.Fatalf("status=%d", resp.StatusCode)
	}
}
