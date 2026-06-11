package model

//nolint:misspell // Uses official Receita Federal field names.

import "github.com/shopspring/decimal"

type Empresa struct {
	CNPJBasico                string          `db:"cnpj_basico" json:"cnpj_basico"`
	RazaoSocial               string          `db:"razao_social" json:"razao_social"`
	NaturezaJuridica          string          `db:"natureza_juridica" json:"natureza_juridica"`
	QualificacaoResponsavel   string          `db:"qualificacao_responsavel" json:"qualificacao_responsavel"`
	CapitalSocial             decimal.Decimal `db:"capital_social" json:"capital_social"`
	PorteEmpresa              string          `db:"porte_empresa" json:"porte_empresa"`
	EnteFederativoResponsavel string          `db:"ente_federativo_responsavel" json:"ente_federativo_responsavel"`
}

type Estabelecimento struct {
	CNPJBasico           string `db:"cnpj_basico" json:"cnpj_basico"`
	CNPJOrdem            string `db:"cnpj_ordem" json:"cnpj_ordem"`
	CNPJDigito           string `db:"cnpj_digito" json:"cnpj_digito"`
	IdentificadorMatriz  int16  `db:"id_matriz_filial" json:"id_matriz_filial"`
	NomeFantasia         string `db:"nome_fantasia" json:"nome_fantasia"`
	SituacaoCadastral    int16  `db:"situacao_cadastral" json:"situacao_cadastral"`
	DataSituacao         *Date  `db:"data_situacao" json:"data_situacao"`
	MotivoSituacao       string `db:"motivo_situacao_cadastral" json:"motivo_situacao_cadastral"`
	NomeCidadeExterior   string `db:"nome_cidade_exterior" json:"nome_cidade_exterior"`
	CodigoPais           string `db:"pais" json:"pais"`
	DataInicioAtividade  *Date  `db:"data_inicio_atividade" json:"data_inicio_atividade"`
	CNAEFiscalPrincipal  string `db:"cnae_fiscal_principal" json:"cnae_fiscal_principal"`
	CNAEFiscalSecundaria string `db:"cnae_fiscal_secundaria" json:"cnae_fiscal_secundaria"`
	TipoLogradouro       string `db:"tipo_logradouro" json:"tipo_logradouro"`
	Logradouro           string `db:"logradouro" json:"logradouro"`
	Numero               string `db:"numero" json:"numero"`
	Complemento          string `db:"complemento" json:"complemento"`
	Bairro               string `db:"bairro" json:"bairro"`
	CEP                  string `db:"cep" json:"cep"`
	UF                   string `db:"uf" json:"uf"`
	CodigoMunicipio      string `db:"municipio" json:"municipio"`
	DDD1                 string `db:"ddd_1" json:"ddd_1"`
	Telefone1            string `db:"telefone_1" json:"telefone_1"`
	DDD2                 string `db:"ddd_2" json:"ddd_2"`
	Telefone2            string `db:"telefone_2" json:"telefone_2"`
	DDDFax               string `db:"ddd_fax" json:"ddd_fax"`
	Fax                  string `db:"fax" json:"fax"`
	Email                string `db:"email" json:"email"`
	SituacaoEspecial     string `db:"situacao_especial" json:"situacao_especial"`
	DataSituacaoEspecial *Date  `db:"data_situacao_especial" json:"data_situacao_especial"`
}

type Socio struct {
	CNPJBasico                string `db:"cnpj_basico" json:"cnpj_basico"`
	IdentificadorSocio        string `db:"identificador_socio" json:"identificador_socio"`
	NomeSocio                 string `db:"nome_socio" json:"nome_socio"`
	CPFCNPJSocio              string `db:"cpf_cnpj_socio" json:"cpf_cnpj_socio"`
	QualificacaoSocio         string `db:"qualificacao_socio" json:"qualificacao_socio"`
	DataEntradaSociedade      *Date  `db:"data_entrada_sociedade" json:"data_entrada_sociedade"`
	Pais                      string `db:"pais" json:"pais"`
	RepresentanteLegal        string `db:"representante_legal" json:"representante_legal"`
	NomeRepresentante         string `db:"nome_representante" json:"nome_representante"`
	QualificacaoRepresentante string `db:"qualificacao_representante" json:"qualificacao_representante"`
	FaixaEtaria               string `db:"faixa_etaria" json:"faixa_etaria"`
}

type Simples struct {
	CNPJBasico          string `db:"cnpj_basico" json:"cnpj_basico"`
	OpcaoSimples        string `db:"opcao_simples" json:"opcao_simples"`
	DataOpcaoSimples    *Date  `db:"data_opcao_simples" json:"data_opcao_simples"`
	DataExclusaoSimples *Date  `db:"data_exclusao_simples" json:"data_exclusao_simples"`
	OpcaoMEI            string `db:"opcao_mei" json:"opcao_mei"`
	DataOpcaoMEI        *Date  `db:"data_opcao_mei" json:"data_opcao_mei"`
	DataExclusaoMEI     *Date  `db:"data_exclusao_mei" json:"data_exclusao_mei"`
}
