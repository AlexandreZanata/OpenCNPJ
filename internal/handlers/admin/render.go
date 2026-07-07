package admin

import (
	"bytes"
	"fmt"
	"html/template"
	"io"
	"io/fs"

	adminstatic "busca-cnpj-2026/internal/static/admin"
	admintmpl "busca-cnpj-2026/internal/templates/admin"
)

// Renderer executes embedded admin HTML templates.
type Renderer struct {
	tmpl *template.Template
}

// NewRenderer parses all admin templates from embed.FS.
func NewRenderer() (*Renderer, error) {
	t, err := template.ParseFS(admintmpl.Files, "*.html")
	if err != nil {
		return nil, fmt.Errorf("parse admin templates: %w", err)
	}
	return &Renderer{tmpl: t}, nil
}

// Render writes template output to w.
func (r *Renderer) Render(w io.Writer, name string, data any) error {
	var buf bytes.Buffer
	if err := r.tmpl.ExecuteTemplate(&buf, name, data); err != nil {
		return fmt.Errorf("execute %s: %w", name, err)
	}
	_, err := io.Copy(w, &buf)
	return err
}

// MustRenderer panics if templates fail to parse (startup).
func MustRenderer() *Renderer {
	r, err := NewRenderer()
	if err != nil {
		panic(err)
	}
	return r
}

// StaticFS returns the embedded CSS filesystem root.
func StaticFS() (fs.FS, error) {
	return fs.Sub(adminstatic.Files, ".")
}
