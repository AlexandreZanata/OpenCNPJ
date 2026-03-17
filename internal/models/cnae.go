package models

import "time"

type CNAE struct {
	Codigo    string    `json:"codigo" db:"codigo"`
	Descricao string    `json:"descricao" db:"descricao"`
	Secao     string    `json:"secao" db:"secao"`
	Divisao   string    `json:"divisao" db:"divisao"`
	CreatedAt time.Time `json:"created_at" db:"created_at"`
}
