-- name: UpsertUsageDaily :exec
INSERT INTO api_usage_daily (client_id, date, request_count, cnpj_lookup_count)
VALUES ($1, $2, $3, $4)
ON CONFLICT (client_id, date) DO UPDATE SET
    request_count = api_usage_daily.request_count + EXCLUDED.request_count,
    cnpj_lookup_count = api_usage_daily.cnpj_lookup_count + EXCLUDED.cnpj_lookup_count;

-- name: GetUsageDaily :one
SELECT client_id, date, request_count, cnpj_lookup_count
FROM api_usage_daily
WHERE client_id = $1 AND date = $2;
