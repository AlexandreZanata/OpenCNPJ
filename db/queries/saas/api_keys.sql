-- name: GetAPIKeyByHash :one
SELECT k.id, k.client_id, k.key_prefix, k.revoked_at, k.expires_at,
       c.status, c.rate_limit_per_min, c.monthly_quota
FROM api_keys k
JOIN api_clients c ON c.id = k.client_id
WHERE k.key_hash = $1 AND k.revoked_at IS NULL;

-- name: GetClientByID :one
SELECT id, name, email, status, monthly_quota, rate_limit_per_min, created_at
FROM api_clients
WHERE id = $1;

-- name: InsertAPIClient :one
INSERT INTO api_clients (name, email, status, monthly_quota, rate_limit_per_min)
VALUES ($1, $2, $3, $4, $5)
RETURNING id, name, email, status, monthly_quota, rate_limit_per_min, created_at;

-- name: InsertAPIKey :one
INSERT INTO api_keys (client_id, key_prefix, key_hash, label, expires_at)
VALUES ($1, $2, $3, $4, $5)
RETURNING id, client_id, key_prefix, label, expires_at, revoked_at, created_at;
