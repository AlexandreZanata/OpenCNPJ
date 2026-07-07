-- name: GetSimplesByBasico :one
SELECT
    opcao_simples,
    data_opcao_simples,
    data_exclusao_simples,
    opcao_mei,
    data_opcao_mei,
    data_exclusao_mei
FROM simples
WHERE cnpj_basico = $1;
