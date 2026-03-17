-- Remove UUID columns and indexes

DROP INDEX IF EXISTS idx_empresas_uuid;
ALTER TABLE empresas DROP COLUMN IF EXISTS id;

DROP INDEX IF EXISTS idx_estabelecimentos_uuid;
ALTER TABLE estabelecimentos DROP COLUMN IF EXISTS uuid_id;

DROP INDEX IF EXISTS idx_socios_uuid;
ALTER TABLE socios DROP COLUMN IF EXISTS uuid_id;

DROP INDEX IF EXISTS idx_simples_uuid;
ALTER TABLE simples DROP COLUMN IF EXISTS uuid_id;
