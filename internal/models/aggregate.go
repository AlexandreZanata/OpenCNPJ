package models

import "database/sql"

// EmpresaFull is empresa with resolved reference descriptions.
type EmpresaFull struct {
	Empresa
	NaturezaDescricao     sql.NullString `json:"natureza_descricao,omitempty"`
	QualificacaoDescricao sql.NullString `json:"qualificacao_descricao,omitempty"`
}

// EmpresaAggregate is empresa plus all related records in the database.
type EmpresaAggregate struct {
	EmpresaFull
	Estabelecimentos []EstabelecimentoCompleto `json:"estabelecimentos"`
	Socios           []Socio                   `json:"socios"`
	Simples          *Simples                  `json:"simples,omitempty"`
}

// EstabelecimentoSearchResult is one estabelecimento with parent empresa and related data.
type EstabelecimentoSearchResult struct {
	EstabelecimentoCompleto
	Empresa EmpresaFull `json:"empresa"`
	Socios  []Socio     `json:"socios"`
	Simples *Simples    `json:"simples,omitempty"`
}
