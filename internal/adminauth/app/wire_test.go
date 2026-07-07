package app_test

import (
	"context"
	"testing"

	"github.com/alicebob/miniredis/v2"
	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/adminauth/app"
	"busca-cnpj-2026/internal/config"
)

func TestWireAdminDisabled(t *testing.T) {
	_, err := app.Wire(context.Background(), nil, nil, config.SaasConfig{AdminEnabled: false})
	if err == nil {
		t.Fatal("expected error when admin disabled")
	}
}

func TestWireRequiresRedis(t *testing.T) {
	t.Setenv("MFA_SECRET_ENCRYPTION_KEY", "MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTIzNDU2Nzg5MDE=")
	t.Setenv("ADMIN_JWT_PRIVATE_KEY_PATH", "/nonexistent/priv.pem")
	_, err := app.Wire(context.Background(), nil, nil, config.SaasConfig{AdminEnabled: true})
	if err == nil {
		t.Fatal("expected error without redis")
	}
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	_, err = app.Wire(context.Background(), nil, rdb, config.SaasConfig{AdminEnabled: true})
	if err == nil {
		t.Fatal("expected error without jwt keys")
	}
}
