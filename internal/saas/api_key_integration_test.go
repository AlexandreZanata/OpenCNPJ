package saas_test

import (
	"context"
	"database/sql"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/golang-migrate/migrate/v4"
	"github.com/golang-migrate/migrate/v4/database/postgres"
	_ "github.com/golang-migrate/migrate/v4/source/file"
	_ "github.com/lib/pq"
	"github.com/google/uuid"
	"github.com/jackc/pgx/v5/pgtype"
	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/testcontainers/testcontainers-go"
	"github.com/testcontainers/testcontainers-go/wait"

	saasdb "busca-cnpj-2026/internal/db/saas"
	"busca-cnpj-2026/internal/saas"
)

func pgClientID(id uuid.UUID) pgtype.UUID {
	return pgtype.UUID{Bytes: id, Valid: true}
}

func TestIntegration_APIKeyLookupAndExplain(t *testing.T) {
	if testing.Short() {
		t.Skip("integration test")
	}
	ctx := context.Background()
	dsn, cleanup := startPostgres(t, ctx)
	defer cleanup()

	pool, err := pgxpool.New(ctx, dsn)
	if err != nil {
		t.Fatal(err)
	}
	defer pool.Close()

	queries := saasdb.New(pool)
	plain, clientID, err := saas.CreateClientKey(ctx, queries, "Gate Co", "gate@test.local", "production", 60, 0)
	if err != nil {
		t.Fatal(err)
	}
	for i := 0; i < 64; i++ {
		extra, genErr := saas.GenerateKey()
		if genErr != nil {
			t.Fatal(genErr)
		}
		_, err = queries.InsertAPIKey(ctx, saasdb.InsertAPIKeyParams{
			ClientID:  pgClientID(clientID),
			KeyPrefix: saas.KeyDisplayPrefix(extra),
			KeyHash:   saas.HashKey(extra),
			Label:     "decoy",
		})
		if err != nil {
			t.Fatal(err)
		}
	}
	if _, err := pool.Exec(ctx, "ANALYZE api_keys"); err != nil {
		t.Fatal(err)
	}

	store := saas.NewKeyStore(queries)
	got, err := store.Authenticate(ctx, plain)
	if err != nil {
		t.Fatalf("authenticate: %v", err)
	}
	if got.ClientID != clientID {
		t.Fatalf("client id mismatch")
	}

	tx, err := pool.Begin(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback(ctx) //nolint:errcheck // read-only EXPLAIN rollback is fine

	if _, err := tx.Exec(ctx, "SET LOCAL enable_seqscan = off"); err != nil {
		t.Fatal(err)
	}
	rows, err := tx.Query(ctx,
		`EXPLAIN SELECT k.id FROM api_keys k
		 WHERE k.key_hash = $1 AND k.revoked_at IS NULL`, saas.HashKey(plain))
	if err != nil {
		t.Fatal(err)
	}
	defer rows.Close()
	plan := ""
	for rows.Next() {
		var line string
		if scanErr := rows.Scan(&line); scanErr != nil {
			t.Fatal(scanErr)
		}
		plan += line + "\n"
	}
	if strings.Contains(plan, "Seq Scan on api_keys") {
		t.Fatalf("sequential scan on api_keys:\n%s", plan)
	}
	if !strings.Contains(plan, "idx_api_keys_hash") {
		t.Fatalf("expected hash index in plan:\n%s", plan)
	}
}

func startPostgres(t *testing.T, ctx context.Context) (dsn string, cleanup func()) {
	t.Helper()
	pgC, err := testcontainers.GenericContainer(ctx, testcontainers.GenericContainerRequest{
		ContainerRequest: testcontainers.ContainerRequest{
			Image:        "postgres:18.4-alpine",
			ExposedPorts: []string{"5432/tcp"},
			Env: map[string]string{
				"POSTGRES_PASSWORD": "test",
				"POSTGRES_DB":       "opencnpj_saas",
			},
			WaitingFor: wait.ForListeningPort("5432/tcp"),
		},
		Started: true,
	})
	if err != nil {
		t.Fatalf("start postgres: %v", err)
	}

	host, err := pgC.Host(ctx)
	if err != nil {
		t.Fatal(err)
	}
	port, err := pgC.MappedPort(ctx, "5432")
	if err != nil {
		t.Fatal(err)
	}
	dsn = fmt.Sprintf("postgres://postgres:test@%s:%s/opencnpj_saas?sslmode=disable", host, port.Port())

	if err := runSaasMigrations(dsn); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	return dsn, func() {
		if termErr := pgC.Terminate(ctx); termErr != nil {
			t.Logf("terminate: %v", termErr)
		}
	}
}

func runSaasMigrations(dsn string) error {
	db, err := sql.Open("postgres", dsn)
	if err != nil {
		return err
	}
	defer db.Close()

	driver, err := postgres.WithInstance(db, &postgres.Config{})
	if err != nil {
		return err
	}
	root, err := repoRoot()
	if err != nil {
		return err
	}
	m, err := migrate.NewWithDatabaseInstance(
		"file://"+filepath.Join(root, "migrations/saas"),
		"postgres",
		driver,
	)
	if err != nil {
		return err
	}
	if err := m.Up(); err != nil && !errors.Is(err, migrate.ErrNoChange) {
		return err
	}
	return nil
}

func repoRoot() (string, error) {
	dir, err := os.Getwd()
	if err != nil {
		return "", err
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir, nil
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			return "", fmt.Errorf("go.mod not found")
		}
		dir = parent
	}
}
