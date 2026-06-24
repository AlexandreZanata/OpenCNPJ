package models

import (
	"database/sql"
	"time"

	"github.com/google/uuid"
)

type Estabelecimento struct {
	ID                        int64          `json:"id" db:"id"`
	UUIDID                    uuid.UUID      `json:"uuid_id" db:"uuid_id"`
	CNPJBasico                string         `json:"cnpj_basico" db:"cnpj_basico"`
	CNPJOrdem                 string         `json:"cnpj_ordem" db:"cnpj_ordem"`
	CNPJDV                    string         `json:"cnpj_dv" db:"cnpj_dv"`
	CNPJCompleto              string         `json:"cnpj_completo" db:"cnpj_completo"`
	IdentificadorMatrizFilial sql.NullString `json:"identificador_matriz_filial" db:"identificador_matriz_filial"`
	NomeFantasia              sql.NullString `json:"nome_fantasia" db:"nome_fantasia"`
	SituacaoCadastral         sql.NullString `json:"situacao_cadastral" db:"situacao_cadastral"`
	DataSituacaoCadastral     sql.NullTime   `json:"data_situacao_cadastral" db:"data_situacao_cadastral"`
	MotivoSituacaoCadastral   sql.NullString `json:"motivo_situacao_cadastral" db:"motivo_situacao_cadastral"`
	NomeCidadeExterior        sql.NullString `json:"nome_cidade_exterior" db:"nome_cidade_exterior"`
	Pais                      sql.NullString `json:"pais" db:"pais"`
	DataInicioAtividade       sql.NullTime   `json:"data_inicio_atividade" db:"data_inicio_atividade"`
	CNAEFiscalPrincipal       sql.NullString `json:"cnae_fiscal_principal" db:"cnae_fiscal_principal"`
	CNAEFiscalSecundaria      sql.NullString `json:"cnae_fiscal_secundaria" db:"cnae_fiscal_secundaria"`
	TipoLogradouro            sql.NullString `json:"tipo_logradouro" db:"tipo_logradouro"`
	Logradouro                sql.NullString `json:"logradouro" db:"logradouro"`
	Numero                    sql.NullString `json:"numero" db:"numero"`
	Complemento               sql.NullString `json:"complemento" db:"complemento"`
	Bairro                    sql.NullString `json:"bairro" db:"bairro"`
	CEP                       sql.NullString `json:"cep" db:"cep"`
	UF                        sql.NullString `json:"uf" db:"uf"`
	Municipio                 sql.NullString `json:"municipio" db:"municipio"`
	DDD1                      sql.NullString `json:"ddd_1" db:"ddd_1"`
	Telefone1                 sql.NullString `json:"telefone_1" db:"telefone_1"`
	DDD2                      sql.NullString `json:"ddd_2" db:"ddd_2"`
	Telefone2                 sql.NullString `json:"telefone_2" db:"telefone_2"`
	DDDFax                    sql.NullString `json:"ddd_fax" db:"ddd_fax"`
	Fax                       sql.NullString `json:"fax" db:"fax"`
	Email                     sql.NullString `json:"email" db:"email"`
	SituacaoEspecial          sql.NullString `json:"situacao_especial" db:"situacao_especial"`
	DataSituacaoEspecial      sql.NullTime   `json:"data_situacao_especial" db:"data_situacao_especial"`
	CreatedAt                 time.Time      `json:"created_at" db:"created_at"`
}

// EstabelecimentoCompleto includes related data.
type EstabelecimentoCompleto struct {
	Estabelecimento
	RazaoSocial     sql.NullString `json:"razao_social" db:"razao_social"`
	CapitalSocial   NullFloat64    `json:"capital_social" db:"capital_social"`
	CNAEDescricao   sql.NullString `json:"cnae_descricao" db:"cnae_descricao"`
	MunicipioNome   sql.NullString `json:"municipio_nome" db:"municipio_nome"`
	MotivoDescricao sql.NullString `json:"motivo_descricao" db:"motivo_descricao"`
	PaisDescricao   sql.NullString `json:"pais_descricao" db:"pais_descricao"`
}
