package admin_test

import (
	"testing"

	"busca-cnpj-2026/internal/handlers/admin"
)

func TestRendererParsesAllTemplates(t *testing.T) {
	r, err := admin.NewRenderer()
	if err != nil {
		t.Fatal(err)
	}
	names := []string{
		"login.html", "mfa.html", "dashboard.html",
		"clients_list.html", "client_new.html", "client_detail.html", "usage.html",
	}
	for _, name := range names {
		var buf []byte
		w := &bytesWriter{&buf}
		if err := r.Render(w, name, map[string]any{
			"Title": "T", "Nav": "dashboard", "ContentTpl": "dashboard-content",
		}); err != nil {
			t.Fatalf("render %s: %v", name, err)
		}
		if len(buf) == 0 {
			t.Fatalf("empty output for %s", name)
		}
	}
}

type bytesWriter struct{ b *[]byte }

func (w *bytesWriter) Write(p []byte) (int, error) {
	*w.b = append(*w.b, p...)
	return len(p), nil
}
