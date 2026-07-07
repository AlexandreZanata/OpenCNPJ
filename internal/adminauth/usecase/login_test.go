package usecase_test

import (
	"context"
	"testing"

	"github.com/alicebob/miniredis/v2"
	"github.com/google/uuid"
	"github.com/redis/go-redis/v9"

	"busca-cnpj-2026/internal/adminauth"
	"busca-cnpj-2026/internal/adminauth/autherr"
	"busca-cnpj-2026/internal/adminauth/bruteforce"
	"busca-cnpj-2026/internal/adminauth/challenge"
	"busca-cnpj-2026/internal/adminauth/password"
	"busca-cnpj-2026/internal/adminauth/usecase"
)

type stubRepo struct {
	row adminauth.AdminRow
}

func (s stubRepo) GetByEmail(context.Context, string) (adminauth.AdminRow, error) {
	return s.row, nil
}

func TestLoginReturnsMFARequired(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	hash, err := password.HashBytes("secret")
	if err != nil {
		t.Fatal(err)
	}
	deps := usecase.LoginDeps{
		Repo: stubRepo{row: adminauth.AdminRow{
			ID: uuid.New(), Email: "a@b.c", PasswordHash: hash, MFAEnabled: true,
		}},
		Guard:   bruteforce.NewGuard(rdb, 5, 15),
		ChStore: challenge.NewStore(rdb, 300),
		Cfg:     adminauth.Config{ChallengeTTLSeconds: 300},
	}
	out, err := usecase.Login(context.Background(), deps, usecase.LoginInput{
		Email: "a@b.c", Password: "secret",
	})
	if err != nil || out.ChallengeID == uuid.Nil {
		t.Fatalf("login err=%v out=%+v", err, out)
	}
}

func TestLoginBadPassword(t *testing.T) {
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatal(err)
	}
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})
	hash, _ := password.HashBytes("secret")
	deps := usecase.LoginDeps{
		Repo: stubRepo{row: adminauth.AdminRow{
			ID: uuid.New(), Email: "a@b.c", PasswordHash: hash, MFAEnabled: true,
		}},
		Guard:   bruteforce.NewGuard(rdb, 5, 15),
		ChStore: challenge.NewStore(rdb, 300),
		Cfg:     adminauth.Config{ChallengeTTLSeconds: 300},
	}
	_, err = usecase.Login(context.Background(), deps, usecase.LoginInput{
		Email: "a@b.c", Password: "wrong",
	})
	if err == nil || err != autherr.ErrInvalidCredentials {
		t.Fatalf("expected invalid credentials, got %v", err)
	}
}
