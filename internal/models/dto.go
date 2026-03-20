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

// ExportRequest represents CSV export request.
type ExportRequest struct {
	Filters         SearchFilters `json:"filters"`
	SelectedColumns []string      `json:"selected_columns"`
	Format          string        `json:"format"` // csv, json
}

// StatsResponse represents statistics response.
type StatsResponse struct {
	CNAE  string `json:"cnae"`
	UF    string `json:"uf"`
	Count int64  `json:"count"`
}

// ErrorResponse represents API error response.
type ErrorResponse struct {
	Error   string `json:"error"`
	Message string `json:"message,omitempty"`
	Code    int    `json:"code"`
}
