-- name: ListSociosByBasico :many
SELECT
    s.nome_socio,
    s.qualificacao_socio,
    COALESCE(q.descricao, '') AS qualificacao_descricao,
    s.data_entrada_sociedade
FROM socios s
LEFT JOIN qualificacoes q ON s.qualificacao_socio = q.codigo
WHERE s.cnpj_basico = $1
ORDER BY s.nome_socio;
