package cnpj_test

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/testcontainers/testcontainers-go"
	"github.com/testcontainers/testcontainers-go/wait"
)

func TestIntegration_EXPLAINCnpjCompletoIndex(t *testing.T) {
	if testing.Short() {
		t.Skip("integration test")
	}
	ctx := context.Background()
	pool, cleanup := startCNPJSchema(t, ctx)
	defer cleanup()

	_, err := pool.Exec(ctx, `
		INSERT INTO empresas (uuid_id, cnpj_basico, razao_social)
		VALUES (gen_random_uuid(), '00000000', 'BANCO DO BRASIL SA');
		INSERT INTO estabelecimentos (
			uuid_id, cnpj_basico, cnpj_ordem, cnpj_dv, nome_fantasia, situacao_cadastral, uf
		) VALUES (gen_random_uuid(), '00000000', '0001', '91', 'BB', '02', 'DF');
		ANALYZE estabelecimentos;
	`)
	if err != nil {
		t.Fatal(err)
	}

	tx, err := pool.Begin(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer tx.Rollback(ctx) //nolint:errcheck // read-only EXPLAIN

	if _, err := tx.Exec(ctx, "SET LOCAL enable_seqscan = off"); err != nil {
		t.Fatal(err)
	}
	rows, err := tx.Query(ctx,
		`EXPLAIN SELECT e.cnpj_completo FROM estabelecimentos e WHERE e.cnpj_completo = $1`,
		"00000000000191")
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
	if strings.Contains(plan, "Seq Scan on estabelecimentos") {
		t.Fatalf("sequential scan:\n%s", plan)
	}
	if !strings.Contains(plan, "idx_estabelecimentos_cnpj_completo") {
		t.Fatalf("expected cnpj_completo index in plan:\n%s", plan)
	}
}

func startCNPJSchema(t *testing.T, ctx context.Context) (*pgxpool.Pool, func()) {
	t.Helper()
	pgC, err := testcontainers.GenericContainer(ctx, testcontainers.GenericContainerRequest{
		ContainerRequest: testcontainers.ContainerRequest{
			Image:        "postgres:18.4-alpine",
			ExposedPorts: []string{"5432/tcp"},
			Env: map[string]string{
				"POSTGRES_PASSWORD": "test",
				"POSTGRES_DB":       "cnpj_test",
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
	dsn := "postgres://postgres:test@" + host + ":" + port.Port() + "/cnpj_test?sslmode=disable"
	pool, err := pgxpool.New(ctx, dsn)
	if err != nil {
		t.Fatal(err)
	}
	schema, err := os.ReadFile(schemaPath(t))
	if err != nil {
		t.Fatal(err)
	}
	if _, err := pool.Exec(ctx, "CREATE EXTENSION IF NOT EXISTS pgcrypto"); err != nil {
		t.Fatal(err)
	}
	if _, err := pool.Exec(ctx, string(schema)); err != nil {
		t.Fatal(err)
	}
	return pool, func() {
		pool.Close()
		_ = pgC.Terminate(ctx)
	}
}

func schemaPath(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		candidate := filepath.Join(dir, "db/schema/cnpj.sql")
		if _, err := os.Stat(candidate); err == nil {
			return candidate
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("db/schema/cnpj.sql not found")
		}
		dir = parent
	}
}
