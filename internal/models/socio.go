package models

import (
	"database/sql"
	"time"

	"github.com/google/uuid"
)

type Socio struct {
	ID                        int64          `json:"id" db:"id"`
	UUIDID                    uuid.UUID      `json:"uuid_id" db:"uuid_id"`
	CNPJBasico                string         `json:"cnpj_basico" db:"cnpj_basico"`
	IdentificadorSocio        sql.NullString `json:"identificador_socio" db:"identificador_socio"`
	NomeSocio                 string         `json:"nome_socio" db:"nome_socio"`
	CPFCNPJSocio              sql.NullString `json:"cpf_cnpj_socio" db:"cpf_cnpj_socio"`
	QualificacaoSocio         sql.NullString `json:"qualificacao_socio" db:"qualificacao_socio"`
	DataEntradaSociedade      sql.NullTime   `json:"data_entrada_sociedade" db:"data_entrada_sociedade"`
	Pais                      sql.NullString `json:"pais" db:"pais"`
	RepresentanteLegal        sql.NullString `json:"representante_legal" db:"representante_legal"`
	NomeRepresentante         sql.NullString `json:"nome_representante" db:"nome_representante"`
	QualificacaoRepresentante sql.NullString `json:"qualificacao_representante" db:"qualificacao_representante"`
	FaixaEtaria               sql.NullString `json:"faixa_etaria" db:"faixa_etaria"`
	CreatedAt                 time.Time      `json:"created_at" db:"created_at"`
}
