-- name: CountAPIClients :one
SELECT COUNT(*)::bigint AS count FROM api_clients;

-- name: SumUsageRequestsToday :one
SELECT COALESCE(SUM(request_count), 0)::bigint AS total
FROM api_usage_daily
WHERE date = CURRENT_DATE;

-- name: ListAPIClients :many
SELECT id, name, email, status, monthly_quota, rate_limit_per_min, created_at
FROM api_clients
ORDER BY created_at DESC
LIMIT $1 OFFSET $2;

-- name: UpdateClientStatus :exec
UPDATE api_clients
SET status = $2
WHERE id = $1;

-- name: ListAPIKeysByClient :many
SELECT id, client_id, key_prefix, label, expires_at, revoked_at, created_at
FROM api_keys
WHERE client_id = $1
ORDER BY created_at DESC;

-- name: RevokeAPIKey :execrows
UPDATE api_keys
SET revoked_at = now()
WHERE id = $1 AND client_id = $2 AND revoked_at IS NULL;

-- name: ListUsageByClient :many
SELECT client_id, date, request_count, cnpj_lookup_count
FROM api_usage_daily
WHERE client_id = $1
ORDER BY date DESC
LIMIT $2;

-- name: ListRecentUsage :many
SELECT u.client_id, c.name AS client_name, u.date, u.request_count, u.cnpj_lookup_count
FROM api_usage_daily u
JOIN api_clients c ON c.id = u.client_id
ORDER BY u.date DESC, u.request_count DESC
LIMIT $1;
