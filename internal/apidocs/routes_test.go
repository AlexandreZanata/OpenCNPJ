package apidocs

import (
	"io"
	"io/fs"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/gofiber/fiber/v2"
)

func testApp(t *testing.T) *fiber.App {
	t.Helper()
	app := fiber.New()
	if err := RegisterRoutes(app); err != nil {
		t.Fatal(err)
	}
	return app
}

func TestDocsIndex200(t *testing.T) {
	app := testApp(t)
	req := httptest.NewRequest(http.MethodGet, "/docs/", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
}

func TestOpenAPISpec200(t *testing.T) {
	app := testApp(t)
	req := httptest.NewRequest(http.MethodGet, "/docs/openapi.yaml", http.NoBody)
	resp, err := app.Test(req)
	if err != nil {
		t.Fatal(err)
	}
	defer resp.Body.Close()
	if resp.StatusCode != http.StatusOK {
		t.Fatalf("status = %d", resp.StatusCode)
	}
	body, _ := io.ReadAll(resp.Body)
	if len(body) < 100 {
		t.Fatal("expected YAML openapi body")
	}
}

func TestEmbeddedOpenAPIMatchesDocs(t *testing.T) {
	root := findRepoRoot(t)
	canonical := filepath.Join(root, "docs", "api", "OPENAPI.yaml")
	want, err := os.ReadFile(canonical)
	if err != nil {
		t.Fatalf("read canonical: %v", err)
	}
	sub, err := fs.Sub(static, "static")
	if err != nil {
		t.Fatal(err)
	}
	got, err := fs.ReadFile(sub, "openapi.yaml")
	if err != nil {
		t.Fatalf("read embedded: %v", err)
	}
	if string(got) != string(want) {
		t.Fatal("embedded openapi.yaml out of sync with docs/api/OPENAPI.yaml")
	}
}

func findRepoRoot(t *testing.T) string {
	t.Helper()
	dir, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	for {
		if _, err := os.Stat(filepath.Join(dir, "go.mod")); err == nil {
			return dir
		}
		parent := filepath.Dir(dir)
		if parent == dir {
			t.Fatal("go.mod not found")
		}
		dir = parent
	}
}
