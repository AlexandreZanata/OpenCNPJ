-- name: GetEstabelecimentoByCNPJ :one
SELECT
    e.cnpj_completo,
    e.cnpj_basico,
    e.nome_fantasia,
    e.situacao_cadastral,
    e.uf,
    COALESCE(m.descricao, '') AS municipio_nome,
    e.municipio AS municipio_codigo,
    e.cnae_fiscal_principal,
    COALESCE(c.descricao, '') AS cnae_descricao,
    e.tipo_logradouro,
    e.logradouro,
    e.numero,
    e.complemento,
    e.bairro,
    e.cep,
    e.ddd_1,
    e.telefone_1,
    e.ddd_2,
    e.telefone_2,
    e.email,
    COALESCE(emp.razao_social, '') AS razao_social
FROM estabelecimentos e
INNER JOIN empresas emp ON e.cnpj_basico = emp.cnpj_basico
LEFT JOIN cnaes c ON e.cnae_fiscal_principal = c.codigo
LEFT JOIN municipios m ON e.municipio = m.codigo
WHERE e.cnpj_completo = $1;
