package parser

import (
	"busca-cnpj-2026/internal/model"
)

const empresasColumns = 7

func ParseEmpresa(line []string, lookups *LookupStore) (model.Empresa, error) {
	if len(line) != empresasColumns {
		return model.Empresa{}, InvalidColumnCountError{
			Entity:   "empresa",
			Expected: empresasColumns,
			Got:      len(line),
		}
	}

	if err := ValidateCNPJBasico(line[0]); err != nil {
		return model.Empresa{}, err
	}
	if lookups != nil && line[2] != "" && !lookups.ExistsNatureza(line[2]) {
		return model.Empresa{}, InvalidFieldError{Field: "natureza_juridica", Value: line[2], Reason: "not found in lookup"}
	}

	capital, err := ParseCapitalSocial(line[4])
	if err != nil {
		return model.Empresa{}, err
	}

	return model.Empresa{
		CNPJBasico:                line[0],
		RazaoSocial:               line[1],
		NaturezaJuridica:          line[2],
		QualificacaoResponsavel:   line[3],
		CapitalSocial:             capital,
		PorteEmpresa:              line[5],
		EnteFederativoResponsavel: line[6],
	}, nil
}
