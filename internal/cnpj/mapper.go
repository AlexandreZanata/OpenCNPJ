package cnpj

import (
	"fmt"
	"strings"

	"github.com/jackc/pgx/v5/pgtype"

	cnpjdb "busca-cnpj-2026/internal/db/cnpj"
)

func buildPublicResponse(
	est cnpjdb.GetEstabelecimentoByCNPJRow,
	socios []cnpjdb.ListSociosByBasicoRow,
	simples *cnpjdb.GetSimplesByBasicoRow,
) PublicResponse {
	resp := PublicResponse{
		CNPJ:              textVal(est.CnpjCompleto),
		RazaoSocial:       est.RazaoSocial,
		NomeFantasia:      textVal(est.NomeFantasia),
		SituacaoCadastral: textVal(est.SituacaoCadastral),
		UF:                textVal(est.Uf),
		Municipio:         est.MunicipioNome,
		CNAEPrincipal: CNAEInfo{
			Codigo:    textVal(est.CnaeFiscalPrincipal),
			Descricao: est.CnaeDescricao,
		},
		Endereco: Endereco{
			Logradouro:  joinStreet(textVal(est.TipoLogradouro), textVal(est.Logradouro)),
			Numero:      textVal(est.Numero),
			Complemento: textVal(est.Complemento),
			Bairro:      textVal(est.Bairro),
			CEP:         textVal(est.Cep),
			UF:          textVal(est.Uf),
			Municipio:   est.MunicipioNome,
		},
		Telefone: formatPhone(textVal(est.Ddd1), textVal(est.Telefone1), textVal(est.Ddd2), textVal(est.Telefone2)),
		Email:    textVal(est.Email),
		Socios:   mapSocios(socios),
	}
	if simples != nil {
		resp.Simples = &SimplesFlags{
			OpcaoSimples:        textVal(simples.OpcaoSimples),
			DataOpcaoSimples:    dateString(simples.DataOpcaoSimples),
			DataExclusaoSimples: dateString(simples.DataExclusaoSimples),
			OpcaoMEI:            textVal(simples.OpcaoMei),
			DataOpcaoMEI:        dateString(simples.DataOpcaoMei),
			DataExclusaoMEI:     dateString(simples.DataExclusaoMei),
		}
	}
	if resp.Socios == nil {
		resp.Socios = []SocioSummary{}
	}
	return resp
}

func mapSocios(rows []cnpjdb.ListSociosByBasicoRow) []SocioSummary {
	out := make([]SocioSummary, 0, len(rows))
	for _, row := range rows {
		qual := row.QualificacaoDescricao
		if qual == "" {
			qual = textVal(row.QualificacaoSocio)
		}
		out = append(out, SocioSummary{
			Nome:                 row.NomeSocio,
			Qualificacao:         qual,
			DataEntradaSociedade: dateString(row.DataEntradaSociedade),
		})
	}
	return out
}

func textVal(t pgtype.Text) string {
	if !t.Valid {
		return ""
	}
	return t.String
}

func dateString(d pgtype.Date) string {
	if !d.Valid {
		return ""
	}
	return d.Time.Format("2006-01-02")
}

func joinStreet(tipo, logradouro string) string {
	if tipo == "" {
		return logradouro
	}
	if logradouro == "" {
		return tipo
	}
	return strings.TrimSpace(tipo + " " + logradouro)
}

func formatPhone(ddd1, tel1, ddd2, tel2 string) string {
	parts := make([]string, 0, 2)
	if ddd1 != "" && tel1 != "" {
		parts = append(parts, fmt.Sprintf("(%s) %s", ddd1, tel1))
	}
	if ddd2 != "" && tel2 != "" {
		parts = append(parts, fmt.Sprintf("(%s) %s", ddd2, tel2))
	}
	return strings.Join(parts, " / ")
}
