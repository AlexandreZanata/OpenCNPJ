-- Phase 12 index gate: api_clients status filter
CREATE INDEX IF NOT EXISTS idx_api_clients_status_active
    ON api_clients (status) WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_api_keys_client_id
    ON api_keys (client_id) WHERE revoked_at IS NULL;
