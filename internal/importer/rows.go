package importer

import (
	"fmt"
	"strconv"

	"busca-cnpj-2026/internal/parser"
)

func empresaRow(line []string, lookups *parser.LookupStore) ([]any, error) {
	item, err := parser.ParseEmpresa(line, lookups)
	if err != nil {
		return nil, err
	}
	return []any{
		cleanText(item.CNPJBasico),
		cleanText(item.RazaoSocial),
		nullIfEmpty(item.NaturezaJuridica),
		nullIfEmpty(item.QualificacaoResponsavel),
		item.CapitalSocial,
		nullIfEmpty(item.PorteEmpresa),
		nullIfEmpty(item.EnteFederativoResponsavel),
	}, nil
}

func estabelecimentoRow(line []string, _ *parser.LookupStore) ([]any, error) {
	item, err := parser.ParseEstabelecimento(line, nil)
	if err != nil {
		return nil, err
	}
	dataSit, err := item.DataSituacao.Value()
	if err != nil {
		return nil, err
	}
	dataIni, err := item.DataInicioAtividade.Value()
	if err != nil {
		return nil, err
	}
	dataEsp, err := item.DataSituacaoEspecial.Value()
	if err != nil {
		return nil, err
	}
	return []any{
		cleanText(item.CNPJBasico),
		cleanText(item.CNPJOrdem),
		cleanText(item.CNPJDigito),
		nullIfEmpty(strconv.Itoa(int(item.IdentificadorMatriz))),
		cleanText(item.NomeFantasia),
		fmt.Sprintf("%02d", item.SituacaoCadastral),
		dataSit,
		nullIfEmpty(item.MotivoSituacao),
		cleanText(item.NomeCidadeExterior),
		nullIfEmpty(item.CodigoPais),
		dataIni,
		nullIfEmpty(item.CNAEFiscalPrincipal),
		cleanText(item.CNAEFiscalSecundaria),
		cleanText(item.TipoLogradouro),
		cleanText(item.Logradouro),
		cleanText(item.Numero),
		cleanText(item.Complemento),
		cleanText(item.Bairro),
		cleanText(item.CEP),
		cleanText(item.UF),
		nullIfEmpty(item.CodigoMunicipio),
		cleanText(item.DDD1),
		cleanText(item.Telefone1),
		cleanText(item.DDD2),
		cleanText(item.Telefone2),
		cleanText(item.DDDFax),
		cleanText(item.Fax),
		cleanText(item.Email),
		cleanText(item.SituacaoEspecial),
		dataEsp,
	}, nil
}

func socioRow(line []string, _ *parser.LookupStore) ([]any, error) {
	item, err := parser.ParseSocio(line, nil)
	if err != nil {
		return nil, err
	}
	dataEnt, err := item.DataEntradaSociedade.Value()
	if err != nil {
		return nil, err
	}
	return []any{
		cleanText(item.CNPJBasico),
		nullIfEmpty(item.IdentificadorSocio),
		cleanText(item.NomeSocio),
		nullIfEmpty(item.CPFCNPJSocio),
		nullIfEmpty(item.QualificacaoSocio),
		dataEnt,
		nullIfEmpty(item.Pais),
		nullIfEmpty(item.RepresentanteLegal),
		cleanText(item.NomeRepresentante),
		nullIfEmpty(item.QualificacaoRepresentante),
		nullIfEmpty(item.FaixaEtaria),
	}, nil
}

func simplesRow(line []string) ([]any, error) {
	item, err := parser.ParseSimples(line)
	if err != nil {
		return nil, err
	}
	dOpt, err := item.DataOpcaoSimples.Value()
	if err != nil {
		return nil, err
	}
	dExc, err := item.DataExclusaoSimples.Value()
	if err != nil {
		return nil, err
	}
	dMEI, err := item.DataOpcaoMEI.Value()
	if err != nil {
		return nil, err
	}
	dMEIExc, err := item.DataExclusaoMEI.Value()
	if err != nil {
		return nil, err
	}
	return []any{
		cleanText(item.CNPJBasico),
		nullIfEmpty(item.OpcaoSimples),
		dOpt,
		dExc,
		nullIfEmpty(item.OpcaoMEI),
		dMEI,
		dMEIExc,
	}, nil
}
