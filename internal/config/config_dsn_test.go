package config

import "testing"

func TestGetMigrateDSNUsesMigratePort(t *testing.T) {
	AppConfig = &Config{
		Database: DatabaseConfig{
			Host:        "localhost",
			Port:        6432,
			MigratePort: 5434,
			User:        "receita_user",
			Password:    "secret",
			Name:        "receita_db",
			SSLMode:     "disable",
		},
	}

	migrateDSN := GetMigrateDSN()
	if migrateDSN != "host=localhost port=5434 user=receita_user password=secret dbname=receita_db sslmode=disable" {
		t.Fatalf("migrate DSN = %q", migrateDSN)
	}

	appDSN := GetDSN()
	if appDSN != "host=localhost port=6432 user=receita_user password=secret dbname=receita_db sslmode=disable" {
		t.Fatalf("app DSN = %q", appDSN)
	}
}
