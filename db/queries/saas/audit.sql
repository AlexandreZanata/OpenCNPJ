-- name: InsertAdminAuditLog :one
INSERT INTO admin_audit_log (admin_id, action, resource_type, resource_id, details)
VALUES ($1, $2, $3, $4, $5)
RETURNING id, admin_id, action, resource_type, resource_id, details, created_at;
