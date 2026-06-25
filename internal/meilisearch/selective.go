package meilisearch

// Active matriz filter (plan 02 Phase 5): situacao_cadastral 02 + headquarters row only.
const (
	activeSituacaoCadastral = "02"
	matrizIdentificador     = "1"
)

// SelectiveEmpresaSQL indexes empresas with an active matriz estabelecimento.
const SelectiveEmpresaSQL = `
		SELECT e.cnpj_basico, e.razao_social
		FROM empresas e
		WHERE EXISTS (
			SELECT 1 FROM estabelecimentos est
			WHERE est.cnpj_basico = e.cnpj_basico
			  AND est.situacao_cadastral = '` + activeSituacaoCadastral + `'
			  AND est.identificador_matriz_filial = '` + matrizIdentificador + `'
		)
		ORDER BY e.cnpj_basico
		LIMIT $1 OFFSET $2`

// SelectiveEstabSQL indexes active matriz estabelecimentos only (~20M target on full dataset).
const SelectiveEstabSQL = `
		SELECT id::text, cnpj_completo, COALESCE(nome_fantasia, ''), situacao_cadastral, COALESCE(uf, '')
		FROM estabelecimentos
		WHERE situacao_cadastral = '` + activeSituacaoCadastral + `'
		  AND identificador_matriz_filial = '` + matrizIdentificador + `'
		ORDER BY id
		LIMIT $1 OFFSET $2`

// FullEstabSQL legacy path (all active estabelecimentos).
const FullEstabSQL = `
		SELECT id::text, cnpj_completo, COALESCE(nome_fantasia, ''), situacao_cadastral, COALESCE(uf, '')
		FROM estabelecimentos
		WHERE situacao_cadastral = '02'
		ORDER BY id
		LIMIT $1 OFFSET $2`

// FullEmpresaSQL legacy path (all empresas).
const FullEmpresaSQL = `
		SELECT cnpj_basico, razao_social
		FROM empresas
		ORDER BY cnpj_basico
		LIMIT $1 OFFSET $2`
