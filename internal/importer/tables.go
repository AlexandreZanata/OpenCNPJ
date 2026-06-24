package importer

import (
	"busca-cnpj-2026/internal/model"
	"busca-cnpj-2026/internal/parser"
)

var (
	empresaCols = []string{
		"cnpj_basico", "razao_social", "natureza_juridica", "qualificacao_responsavel",
		"capital_social", "porte_empresa", "ente_federativo_responsavel",
	}
	estabCols = []string{
		"cnpj_basico", "cnpj_ordem", "cnpj_dv", "identificador_matriz_filial", "nome_fantasia",
		"situacao_cadastral", "data_situacao_cadastral", "motivo_situacao_cadastral",
		"nome_cidade_exterior", "pais", "data_inicio_atividade", "cnae_fiscal_principal",
		"cnae_fiscal_secundaria", "tipo_logradouro", "logradouro", "numero", "complemento",
		"bairro", "cep", "uf", "municipio", "ddd_1", "telefone_1", "ddd_2", "telefone_2",
		"ddd_fax", "fax", "email", "situacao_especial", "data_situacao_especial",
	}
	socioCols = []string{
		"cnpj_basico", "identificador_socio", "nome_socio", "cpf_cnpj_socio",
		"qualificacao_socio", "data_entrada_sociedade", "pais", "representante_legal",
		"nome_representante", "qualificacao_representante", "faixa_etaria",
	}
	simplesCols = []string{
		"cnpj_basico", "opcao_simples", "data_opcao_simples", "data_exclusao_simples",
		"opcao_mei", "data_opcao_mei", "data_exclusao_mei",
	}
)

func buildEmpresaRow(line []string, lookups *parser.LookupStore, percent int) ([]any, bool, error) {
	if len(line) > 0 && !InSample(line[0], percent) {
		return nil, false, nil
	}
	m, err := parser.ParseEmpresa(line, lookups)
	if err != nil {
		return nil, false, nil //nolint:nilerr // skip malformed CSV rows during bulk import
	}
	qual := fkOrNil(lookups.ExistsQualificacao, m.QualificacaoResponsavel)
	return cleanRow([]any{
		sanitize(m.CNPJBasico), sanitize(m.RazaoSocial), sanitize(m.NaturezaJuridica), qual,
		m.CapitalSocial, nullStr(m.PorteEmpresa), nullStr(m.EnteFederativoResponsavel),
	}), true, nil
}

func buildEstabRow(line []string, lookups *parser.LookupStore) ([]any, bool, error) {
	m, err := parser.ParseEstabelecimento(line, nil)
	if err != nil {
		return nil, false, nil //nolint:nilerr // skip malformed CSV rows during bulk import
	}
	return cleanRow([]any{
		sanitize(m.CNPJBasico), sanitize(m.CNPJOrdem), sanitize(m.CNPJDigito),
		int16Str(m.IdentificadorMatriz), nullStr(m.NomeFantasia),
		int16Pad2(m.SituacaoCadastral), dateVal(m.DataSituacao), fkOrNil(lookups.ExistsMotivo, m.MotivoSituacao),
		nullStr(m.NomeCidadeExterior), fkOrNil(lookups.ExistsPais, m.CodigoPais), dateVal(m.DataInicioAtividade),
		fkOrNil(lookups.ExistsCNAE, m.CNAEFiscalPrincipal), nullStr(m.CNAEFiscalSecundaria), nullStr(m.TipoLogradouro),
		nullStr(m.Logradouro), nullStr(m.Numero), nullStr(m.Complemento), nullStr(m.Bairro),
		nullStr(m.CEP), nullStr(m.UF), fkOrNil(lookups.ExistsMunicipio, m.CodigoMunicipio),
		nullStr(m.DDD1), nullStr(m.Telefone1),
		nullStr(m.DDD2), nullStr(m.Telefone2), nullStr(m.DDDFax), nullStr(m.Fax), nullStr(m.Email),
		nullStr(m.SituacaoEspecial), dateVal(m.DataSituacaoEspecial),
	}), true, nil
}

func buildSocioRow(line []string, lookups *parser.LookupStore) ([]any, bool, error) {
	m, err := parser.ParseSocio(line, nil)
	if err != nil {
		return nil, false, nil //nolint:nilerr // skip malformed CSV rows during bulk import
	}
	return socioToRow(m, lookups), true, nil
}

func buildSimplesRow(line []string, _ *parser.LookupStore) ([]any, bool, error) {
	m, err := parser.ParseSimples(line)
	if err != nil {
		return nil, false, nil //nolint:nilerr // skip malformed CSV rows during bulk import
	}
	return cleanRow([]any{
		sanitize(m.CNPJBasico), nullStr(m.OpcaoSimples), dateVal(m.DataOpcaoSimples), dateVal(m.DataExclusaoSimples),
		nullStr(m.OpcaoMEI), dateVal(m.DataOpcaoMEI), dateVal(m.DataExclusaoMEI),
	}), true, nil
}

func socioToRow(m model.Socio, lookups *parser.LookupStore) []any {
	return cleanRow([]any{
		sanitize(m.CNPJBasico), nullStr(m.IdentificadorSocio), sanitize(m.NomeSocio), nullStr(m.CPFCNPJSocio),
		fkOrNil(lookups.ExistsQualificacao, m.QualificacaoSocio), dateVal(m.DataEntradaSociedade),
		fkOrNil(lookups.ExistsPais, m.Pais),
		nullStr(m.RepresentanteLegal), nullStr(m.NomeRepresentante), nullStr(m.QualificacaoRepresentante),
		nullStr(m.FaixaEtaria),
	})
}
