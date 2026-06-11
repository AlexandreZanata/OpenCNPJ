package parser

import "busca-cnpj-2026/internal/model"

const sociosColumns = 11

func ParseSocio(line []string, lookups *LookupStore) (model.Socio, error) {
	if len(line) != sociosColumns {
		return model.Socio{}, InvalidColumnCountError{
			Entity: "socio", Expected: sociosColumns, Got: len(line),
		}
	}
	if err := ValidateCNPJBasico(line[0]); err != nil {
		return model.Socio{}, err
	}
	dataEntrada, err := model.ParseDate(line[5])
	if err != nil {
		return model.Socio{}, err
	}
	_ = lookups
	return model.Socio{
		CNPJBasico: line[0], IdentificadorSocio: line[1], NomeSocio: line[2],
		CPFCNPJSocio: line[3], QualificacaoSocio: line[4], DataEntradaSociedade: dataEntrada,
		Pais: line[6], RepresentanteLegal: line[7], NomeRepresentante: line[8],
		QualificacaoRepresentante: line[9], FaixaEtaria: line[10],
	}, nil
}
