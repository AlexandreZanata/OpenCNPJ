-- name: GetAdminUserByEmail :one
SELECT id, email, password_hash, mfa_enabled, created_at, updated_at
FROM admin_users
WHERE email = $1;

-- name: UpsertAdminUser :one
INSERT INTO admin_users (email, password_hash, mfa_enabled)
VALUES ($1, $2, $3)
ON CONFLICT (email) DO UPDATE
SET password_hash = EXCLUDED.password_hash,
    mfa_enabled = EXCLUDED.mfa_enabled,
    updated_at = now()
RETURNING id, email, password_hash, mfa_enabled, created_at, updated_at;

-- name: SetAdminMFAEnabled :exec
UPDATE admin_users
SET mfa_enabled = $2, updated_at = now()
WHERE id = $1;

-- name: UpsertAdminMFASecret :exec
INSERT INTO admin_mfa_secrets (admin_id, secret_encrypted)
VALUES ($1, $2)
ON CONFLICT (admin_id) DO UPDATE
SET secret_encrypted = EXCLUDED.secret_encrypted;

-- name: GetAdminMFASecret :one
SELECT admin_id, secret_encrypted, created_at
FROM admin_mfa_secrets
WHERE admin_id = $1;

-- name: InsertAdminRefreshToken :one
INSERT INTO admin_refresh_tokens (admin_id, token_hash, expires_at)
VALUES ($1, $2, $3)
RETURNING id, admin_id, token_hash, expires_at, revoked_at, created_at;

-- name: GetValidRefreshToken :one
SELECT id, admin_id, token_hash, expires_at, revoked_at, created_at
FROM admin_refresh_tokens
WHERE token_hash = $1
  AND revoked_at IS NULL
  AND expires_at > now();

-- name: RevokeRefreshToken :exec
UPDATE admin_refresh_tokens
SET revoked_at = now()
WHERE id = $1;
