-- Rollback hash partition migration and UUID standardization.

DROP TABLE IF EXISTS estabelecimentos CASCADE;
DROP TABLE IF EXISTS socios CASCADE;

ALTER TABLE IF EXISTS estabelecimentos_legacy_range RENAME TO estabelecimentos;
ALTER TABLE IF EXISTS socios_legacy_range RENAME TO socios;

DROP INDEX IF EXISTS idx_motivos_uuid_id;
DROP INDEX IF EXISTS idx_municipios_uuid_id;
DROP INDEX IF EXISTS idx_naturezas_uuid_id;
DROP INDEX IF EXISTS idx_paises_uuid_id;
DROP INDEX IF EXISTS idx_qualificacoes_uuid_id;
DROP INDEX IF EXISTS idx_cnaes_uuid_id;

ALTER TABLE IF EXISTS motivos DROP COLUMN IF EXISTS uuid_id;
ALTER TABLE IF EXISTS municipios DROP COLUMN IF EXISTS uuid_id;
ALTER TABLE IF EXISTS naturezas DROP COLUMN IF EXISTS uuid_id;
ALTER TABLE IF EXISTS paises DROP COLUMN IF EXISTS uuid_id;
ALTER TABLE IF EXISTS qualificacoes DROP COLUMN IF EXISTS uuid_id;
ALTER TABLE IF EXISTS cnaes DROP COLUMN IF EXISTS uuid_id;
