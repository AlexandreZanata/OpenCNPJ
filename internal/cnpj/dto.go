package cnpj

// PublicResponse is the customer-facing CNPJ lookup payload.
type PublicResponse struct {
	CNPJ              string         `json:"cnpj"`
	RazaoSocial       string         `json:"razao_social"`
	NomeFantasia      string         `json:"nome_fantasia,omitempty"`
	SituacaoCadastral string         `json:"situacao_cadastral,omitempty"`
	UF                string         `json:"uf,omitempty"`
	Municipio         string         `json:"municipio,omitempty"`
	CNAEPrincipal     CNAEInfo       `json:"cnae_principal,omitempty"`
	Endereco          Endereco       `json:"endereco,omitempty"`
	Telefone          string         `json:"telefone,omitempty"`
	Email             string         `json:"email,omitempty"`
	Socios            []SocioSummary `json:"socios"`
	Simples           *SimplesFlags  `json:"simples,omitempty"`
}

// CNAEInfo holds principal activity code and description.
type CNAEInfo struct {
	Codigo    string `json:"codigo,omitempty"`
	Descricao string `json:"descricao,omitempty"`
}

// Endereco is a formatted establishment address.
type Endereco struct {
	Logradouro  string `json:"logradouro,omitempty"`
	Numero      string `json:"numero,omitempty"`
	Complemento string `json:"complemento,omitempty"`
	Bairro      string `json:"bairro,omitempty"`
	CEP         string `json:"cep,omitempty"`
	UF          string `json:"uf,omitempty"`
	Municipio   string `json:"municipio,omitempty"`
}

// SocioSummary is a trimmed partner record for public API consumers.
type SocioSummary struct {
	Nome                 string `json:"nome"`
	Qualificacao         string `json:"qualificacao,omitempty"`
	DataEntradaSociedade string `json:"data_entrada_sociedade,omitempty"`
}

// SimplesFlags reports Simples Nacional / MEI options.
type SimplesFlags struct {
	OpcaoSimples        string `json:"opcao_simples,omitempty"`
	DataOpcaoSimples    string `json:"data_opcao_simples,omitempty"`
	DataExclusaoSimples string `json:"data_exclusao_simples,omitempty"`
	OpcaoMEI            string `json:"opcao_mei,omitempty"`
	DataOpcaoMEI        string `json:"data_opcao_mei,omitempty"`
	DataExclusaoMEI     string `json:"data_exclusao_mei,omitempty"`
}
