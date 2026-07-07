-- name: GetEmpresaByBasico :one
SELECT
    emp.cnpj_basico,
    emp.razao_social,
    emp.natureza_juridica,
    COALESCE(n.descricao, '') AS natureza_descricao,
    emp.porte_empresa,
    emp.capital_social
FROM empresas emp
LEFT JOIN naturezas n ON emp.natureza_juridica = n.codigo
WHERE emp.cnpj_basico = $1;
