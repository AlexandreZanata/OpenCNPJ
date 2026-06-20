package models

// SearchFilters represents filters for search queries.
type SearchFilters struct {
	UUIDID            string
	CNPJBasico        string
	CNPJCompleto      string
	RazaoSocial       string
	NomeFantasia      string
	CNAEPrincipal     string
	CNAESecundaria    string
	UF                string
	Municipio         string
	SituacaoCadastral string
	PorteEmpresa      string
	CEP               string
	NaturezaJuridica  string
	CapitalSocialMin  *float64
	CapitalSocialMax  *float64
	Limit             int
	Offset            int
}

// SearchResponse represents paginated search response.
type SearchResponse struct {
	Data    interface{} `json:"data"`
	Total   int64       `json:"total"`
	Limit   int         `json:"limit"`
	Offset  int         `json:"offset"`
	HasMore bool        `json:"has_more"`
}

// PhoneExportRequest exports establishment phone contacts by category and filters.
type PhoneExportRequest struct {
	Category      string `json:"category"`
	CNAEPrincipal string `json:"cnae"`
	UF            string `json:"uf"`
	Municipio     string `json:"municipio"`
	MunicipioNome string `json:"municipio_nome"`
	NomeFantasia  string `json:"nome_fantasia"`
	OnlyActive    *bool  `json:"only_active"`
	Limit         int    `json:"limit"`
	Format        string `json:"format"` // csv, txt
}

// ExportCategory describes a preset business segment for phone export.
type ExportCategory struct {
	Key         string   `json:"key"`
	Label       string   `json:"label"`
	Description string   `json:"description"`
	CNAECodes   []string `json:"cnae_codes"`
}

// ExportRequest represents CSV export request.
type ExportRequest struct {
	Filters         SearchFilters `json:"filters"`
	SelectedColumns []string      `json:"selected_columns"`
	Format          string        `json:"format"` // csv, json
}

// StatsResponse represents statistics response.
type StatsResponse struct {
	CNAE  string `json:"cnae,omitempty"`
	UF    string `json:"uf,omitempty"`
	Count int64  `json:"count"`
}

// CNAEUFBreakdown groups UF counts for one CNAE.
type CNAEUFBreakdown struct {
	CNAE string          `json:"cnae"`
	ByUF []StatsResponse `json:"by_uf"`
}

// AnalyticsSummaryResponse bundles pre-aggregated analytics for the portal.
type AnalyticsSummaryResponse struct {
	Source      string          `json:"source"`
	RefreshedAt string          `json:"refreshed_at,omitempty"`
	ByUF        []StatsResponse `json:"by_uf"`
	TopCNAE     []StatsResponse `json:"top_cnae"`
	TopCNAEUF   CNAEUFBreakdown `json:"top_cnae_uf"`
}

// ErrorResponse represents API error response.
type ErrorResponse struct {
	Error   string `json:"error"`
	Message string `json:"message,omitempty"`
	Code    int    `json:"code"`
}
