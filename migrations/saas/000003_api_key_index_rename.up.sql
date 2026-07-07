-- Align index name with Phase 3 EXPLAIN gate (idx_api_keys_hash_active)
ALTER INDEX IF EXISTS idx_api_keys_hash RENAME TO idx_api_keys_hash_active;
