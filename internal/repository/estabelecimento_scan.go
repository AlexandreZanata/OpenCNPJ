package repository

import (
	"database/sql"
	"fmt"

	"busca-cnpj-2026/internal/models"
)

const estabelecimentoCompletoSelect = `
		e.id, e.uuid_id, e.cnpj_basico, e.cnpj_ordem, e.cnpj_dv, e.cnpj_completo,
		e.identificador_matriz_filial, e.nome_fantasia, e.situacao_cadastral,
		e.data_situacao_cadastral, e.motivo_situacao_cadastral,
		e.nome_cidade_exterior, e.pais, e.data_inicio_atividade,
		e.cnae_fiscal_principal, e.cnae_fiscal_secundaria,
		e.tipo_logradouro, e.logradouro, e.numero, e.complemento,
		e.bairro, e.cep, e.uf, e.municipio,
		e.ddd_1, e.telefone_1, e.ddd_2, e.telefone_2,
		e.ddd_fax, e.fax, e.email, e.situacao_especial,
		e.data_situacao_especial, e.created_at,
		emp.razao_social, emp.capital_social,
		c.descricao AS cnae_descricao,
		m.descricao AS municipio_nome,
		mot.descricao AS motivo_descricao,
		pa.descricao AS pais_descricao`

const estabelecimentoCompletoFrom = `
		FROM estabelecimentos e
		INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
		LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
		LEFT JOIN municipios m ON e.municipio = m.codigo
		LEFT JOIN motivos mot ON e.motivo_situacao_cadastral = mot.codigo
		LEFT JOIN paises pa ON e.pais = pa.codigo`

func scanEstabelecimentoCompleto(scanner interface {
	Scan(dest ...any) error
}, est *models.EstabelecimentoCompleto,
) error {
	err := scanner.Scan(
		&est.ID,
		&est.UUIDID,
		&est.CNPJBasico,
		&est.CNPJOrdem,
		&est.CNPJDV,
		&est.CNPJCompleto,
		&est.IdentificadorMatrizFilial,
		&est.NomeFantasia,
		&est.SituacaoCadastral,
		&est.DataSituacaoCadastral,
		&est.MotivoSituacaoCadastral,
		&est.NomeCidadeExterior,
		&est.Pais,
		&est.DataInicioAtividade,
		&est.CNAEFiscalPrincipal,
		&est.CNAEFiscalSecundaria,
		&est.TipoLogradouro,
		&est.Logradouro,
		&est.Numero,
		&est.Complemento,
		&est.Bairro,
		&est.CEP,
		&est.UF,
		&est.Municipio,
		&est.DDD1,
		&est.Telefone1,
		&est.DDD2,
		&est.Telefone2,
		&est.DDDFax,
		&est.Fax,
		&est.Email,
		&est.SituacaoEspecial,
		&est.DataSituacaoEspecial,
		&est.CreatedAt,
		&est.RazaoSocial,
		&est.CapitalSocial,
		&est.CNAEDescricao,
		&est.MunicipioNome,
		&est.MotivoDescricao,
		&est.PaisDescricao,
	)
	if err != nil {
		return fmt.Errorf("scan estabelecimento completo: %w", err)
	}
	return nil
}

func scanEstabelecimentoCompletoWithScore(
	scanner interface{ Scan(dest ...any) error },
	est *models.EstabelecimentoCompleto,
	score *float64,
) error {
	err := scanner.Scan(
		&est.ID,
		&est.UUIDID,
		&est.CNPJBasico,
		&est.CNPJOrdem,
		&est.CNPJDV,
		&est.CNPJCompleto,
		&est.IdentificadorMatrizFilial,
		&est.NomeFantasia,
		&est.SituacaoCadastral,
		&est.DataSituacaoCadastral,
		&est.MotivoSituacaoCadastral,
		&est.NomeCidadeExterior,
		&est.Pais,
		&est.DataInicioAtividade,
		&est.CNAEFiscalPrincipal,
		&est.CNAEFiscalSecundaria,
		&est.TipoLogradouro,
		&est.Logradouro,
		&est.Numero,
		&est.Complemento,
		&est.Bairro,
		&est.CEP,
		&est.UF,
		&est.Municipio,
		&est.DDD1,
		&est.Telefone1,
		&est.DDD2,
		&est.Telefone2,
		&est.DDDFax,
		&est.Fax,
		&est.Email,
		&est.SituacaoEspecial,
		&est.DataSituacaoEspecial,
		&est.CreatedAt,
		&est.RazaoSocial,
		&est.CapitalSocial,
		&est.CNAEDescricao,
		&est.MunicipioNome,
		&est.MotivoDescricao,
		&est.PaisDescricao,
		score,
	)
	if err != nil {
		return fmt.Errorf("scan estabelecimento completo with score: %w", err)
	}
	return nil
}

func scanEstabelecimentoRows(rows *sql.Rows) ([]models.EstabelecimentoCompleto, error) {
	out := make([]models.EstabelecimentoCompleto, 0)
	for rows.Next() {
		var est models.EstabelecimentoCompleto
		if err := scanEstabelecimentoCompleto(rows, &est); err != nil {
			return nil, err
		}
		out = append(out, est)
	}
	return out, rows.Err()
}
