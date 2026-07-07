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
}
