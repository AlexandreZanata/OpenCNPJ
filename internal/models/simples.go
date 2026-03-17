package models

import (
	"database/sql"

	"github.com/google/uuid"
)

type Simples struct {
	UUIDID                uuid.UUID    `json:"uuid_id" db:"uuid_id"`
	CNPJBasico           string       `json:"cnpj_basico" db:"cnpj_basico"`
	OpcaoSimples         sql.NullString `json:"opcao_simples" db:"opcao_simples"`
	DataOpcaoSimples     sql.NullTime   `json:"data_opcao_simples" db:"data_opcao_simples"`
	DataExclusaoSimples  sql.NullTime   `json:"data_exclusao_simples" db:"data_exclusao_simples"`
	OpcaoMEI             sql.NullString `json:"opcao_mei" db:"opcao_mei"`
	DataOpcaoMEI         sql.NullTime   `json:"data_opcao_mei" db:"data_opcao_mei"`
	DataExclusaoMEI      sql.NullTime   `json:"data_exclusao_mei" db:"data_exclusao_mei"`
}
