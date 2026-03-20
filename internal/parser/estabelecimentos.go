package parser

import (
	"strconv"

	"busca-cnpj-2026/internal/model"
)

const estabelecimentosColumns = 30

func ParseEstabelecimento(line []string, lookups *LookupStore) (model.Estabelecimento, error) {
	if len(line) != estabelecimentosColumns {
		return model.Estabelecimento{}, InvalidColumnCountError{
			Entity:   "estabelecimento",
			Expected: estabelecimentosColumns,
			Got:      len(line),
		}
	}

	if err := ValidateCNPJBasico(line[0]); err != nil {
		return model.Estabelecimento{}, err
	}

	idMatriz, err := strconv.ParseInt(line[3], 10, 16)
	if err != nil {
		return model.Estabelecimento{}, InvalidFieldError{Field: "id_matriz_filial", Value: line[3], Reason: err.Error()}
	}
	sitCad, err := strconv.ParseInt(line[5], 10, 16)
	if err != nil {
		return model.Estabelecimento{}, InvalidFieldError{Field: "situacao_cadastral", Value: line[5], Reason: err.Error()}
	}
	dataSituacao, err := model.ParseDate(line[6])
	if err != nil {
		return model.Estabelecimento{}, err
	}
	dataInicio, err := model.ParseDate(line[10])
	if err != nil {
		return model.Estabelecimento{}, err
	}
	dataSitEspecial, err := model.ParseDate(line[29])
	if err != nil {
		return model.Estabelecimento{}, err
	}

	if lookups != nil {
		if line[9] != "" && !lookups.ExistsPais(line[9]) {
			return model.Estabelecimento{}, InvalidFieldError{Field: "pais", Value: line[9], Reason: "not found in lookup"}
		}
		if line[11] != "" && !lookups.ExistsCNAE(line[11]) {
			return model.Estabelecimento{}, InvalidFieldError{
				Field:  "cnae_fiscal_principal",
				Value:  line[11],
				Reason: "not found in lookup",
			}
		}
		if line[20] != "" && !lookups.ExistsMunicipio(line[20]) {
			return model.Estabelecimento{}, InvalidFieldError{Field: "municipio", Value: line[20], Reason: "not found in lookup"}
		}
	}

	return model.Estabelecimento{
		CNPJBasico:           line[0],
		CNPJOrdem:            line[1],
		CNPJDigito:           line[2],
		IdentificadorMatriz:  int16(idMatriz),
		NomeFantasia:         line[4],
		SituacaoCadastral:    int16(sitCad),
		DataSituacao:         dataSituacao,
		MotivoSituacao:       line[7],
		NomeCidadeExterior:   line[8],
		CodigoPais:           line[9],
		DataInicioAtividade:  dataInicio,
		CNAEFiscalPrincipal:  line[11],
		CNAEFiscalSecundaria: line[12],
		TipoLogradouro:       line[13],
		Logradouro:           line[14],
		Numero:               line[15],
		Complemento:          line[16],
		Bairro:               line[17],
		CEP:                  line[18],
		UF:                   line[19],
		CodigoMunicipio:      line[20],
		DDD1:                 line[21],
		Telefone1:            line[22],
		DDD2:                 line[23],
		Telefone2:            line[24],
		DDDFax:               line[25],
		Fax:                  line[26],
		Email:                line[27],
		SituacaoEspecial:     line[28],
		DataSituacaoEspecial: dataSitEspecial,
	}, nil
}
