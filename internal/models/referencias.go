package models

import "github.com/google/uuid"

// Motivo represents motivo_situacao_cadastral.
type Motivo struct {
	UUIDID    uuid.UUID `json:"uuid_id" db:"uuid_id"`
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}

// Municipio represents municipio.
type Municipio struct {
	UUIDID    uuid.UUID `json:"uuid_id" db:"uuid_id"`
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
	UF        string `json:"uf" db:"uf"`
}

// Natureza represents natureza_juridica.
type Natureza struct {
	UUIDID    uuid.UUID `json:"uuid_id" db:"uuid_id"`
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}

// Pais represents pais.
type Pais struct {
	UUIDID    uuid.UUID `json:"uuid_id" db:"uuid_id"`
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}

// Qualificacao represents qualificacao.
type Qualificacao struct {
	UUIDID    uuid.UUID `json:"uuid_id" db:"uuid_id"`
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}
