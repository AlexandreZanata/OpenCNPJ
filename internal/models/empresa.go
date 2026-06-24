package models

//nolint:misspell // Uses official Receita Federal field names.

import (
	"database/sql"
	"time"

	"github.com/google/uuid"
)

type Empresa struct {
	UUIDID                    uuid.UUID       `json:"uuid_id" db:"uuid_id"`
	CNPJBasico                string          `json:"cnpj_basico" db:"cnpj_basico"`
	RazaoSocial               string          `json:"razao_social" db:"razao_social"`
	NaturezaJuridica          sql.NullString  `json:"natureza_juridica" db:"natureza_juridica"`
	QualificacaoResponsavel   sql.NullString  `json:"qualificacao_responsavel" db:"qualificacao_responsavel"`
	CapitalSocial             NullFloat64     `json:"capital_social" db:"capital_social"`
	PorteEmpresa              sql.NullString  `json:"porte_empresa" db:"porte_empresa"`
	EnteFederativoResponsavel sql.NullString  `json:"ente_federativo_responsavel" db:"ente_federativo_responsavel"`
	CreatedAt                 time.Time       `json:"created_at" db:"created_at"`
	UpdatedAt                 time.Time       `json:"updated_at" db:"updated_at"`
}
