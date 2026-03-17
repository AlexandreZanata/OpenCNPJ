package models

// Motivo represents motivo_situacao_cadastral
type Motivo struct {
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}

// Municipio represents municipio
type Municipio struct {
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
	UF        string `json:"uf" db:"uf"`
}

// Natureza represents natureza_juridica
type Natureza struct {
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}

// Pais represents pais
type Pais struct {
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}

// Qualificacao represents qualificacao
type Qualificacao struct {
	Codigo    string `json:"codigo" db:"codigo"`
	Descricao string `json:"descricao" db:"descricao"`
}
