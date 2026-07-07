package admin

import (
	"bytes"
	"strings"
	"testing"
)

func TestShellIncludesAPIDocsURL(t *testing.T) {
	h := &Handler{Deps: Deps{DocsPublicURL: "https://example.com/docs"}}
	data := h.shell("Dashboard", "dashboard", "dashboard-content", true)
	if data.APIDocsURL != "https://example.com/docs" {
		t.Fatalf("url = %q", data.APIDocsURL)
	}
}

func TestLayoutRendersAPIDocsLink(t *testing.T) {
	r, err := NewRenderer()
	if err != nil {
		t.Fatal(err)
	}
	var buf bytes.Buffer
	err = r.Render(&buf, "dashboard.html", dashboardPage{
		LayoutData: LayoutData{
			Title: "Dashboard", Nav: "dashboard", ContentTpl: "dashboard-content",
			APIDocsURL: "https://example.com/api-docs",
		},
	})
	if err != nil {
		t.Fatal(err)
	}
	out := buf.String()
	if !strings.Contains(out, "API docs") || !strings.Contains(out, "https://example.com/api-docs") {
		t.Fatalf("layout missing API docs link: %s", out)
	}
}
