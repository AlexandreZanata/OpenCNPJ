DROP INDEX IF EXISTS idx_estabelecimentos_busca_fts;
DROP INDEX IF EXISTS idx_empresas_busca_fts;
ALTER TABLE estabelecimentos DROP COLUMN IF EXISTS busca;
ALTER TABLE empresas DROP COLUMN IF EXISTS busca;
