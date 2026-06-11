package parser

import "busca-cnpj-2026/internal/model"

const simplesColumns = 7

func ParseSimples(line []string) (model.Simples, error) {
	if len(line) != simplesColumns {
		return model.Simples{}, InvalidColumnCountError{
			Entity: "simples", Expected: simplesColumns, Got: len(line),
		}
	}
	if err := ValidateCNPJBasico(line[0]); err != nil {
		return model.Simples{}, err
	}
	dOpt, err := model.ParseDate(line[2])
	if err != nil {
		return model.Simples{}, err
	}
	dExc, err := model.ParseDate(line[3])
	if err != nil {
		return model.Simples{}, err
	}
	dMEI, err := model.ParseDate(line[5])
	if err != nil {
		return model.Simples{}, err
	}
	dMEIExc, err := model.ParseDate(line[6])
	if err != nil {
		return model.Simples{}, err
	}
	return model.Simples{
		CNPJBasico: line[0], OpcaoSimples: line[1], DataOpcaoSimples: dOpt,
		DataExclusaoSimples: dExc, OpcaoMEI: line[4], DataOpcaoMEI: dMEI, DataExclusaoMEI: dMEIExc,
	}, nil
}
