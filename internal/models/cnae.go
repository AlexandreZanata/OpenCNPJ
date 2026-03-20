package models

import (
	"time"

	"github.com/google/uuid"
)

type CNAE struct {
	UUIDID    uuid.UUID `json:"uuid_id" db:"uuid_id"`
	Codigo    string    `json:"codigo" db:"codigo"`
	Descricao string    `json:"descricao" db:"descricao"`
	Secao     string    `json:"secao" db:"secao"`
	Divisao   string    `json:"divisao" db:"divisao"`
	CreatedAt time.Time `json:"created_at" db:"created_at"`
}
