package admin

// Handler serves server-rendered admin pages.
type Handler struct {
	Deps
}

// NewHandler builds the admin panel handler.
func NewHandler(d Deps) *Handler {
	if d.Renderer == nil {
		d.Renderer = MustRenderer()
	}
	return &Handler{Deps: d}
}

// LayoutData is shared shell template data.
type LayoutData struct {
	Title       string
	Nav         string
	ContentTpl  string
	RefreshMeta bool
	Flash       string
	APIDocsURL  string
}

func (h *Handler) shell(title, nav, contentTpl string, refresh bool) LayoutData {
	return LayoutData{
		Title:       title,
		Nav:         nav,
		ContentTpl:  contentTpl,
		RefreshMeta: refresh,
		APIDocsURL:  h.DocsPublicURL,
	}
}
