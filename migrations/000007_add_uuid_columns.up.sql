-- Add UUID v7 columns to all main tables using PostgreSQL 18 native uuidv7() function
-- Note: PostgreSQL 18 uses uuidv7() not uuid_generate_v7()
-- Note: For partitioned tables, UNIQUE constraints must include partitioning columns
-- So we use UNIQUE INDEX instead of UNIQUE constraint for partitioned tables

-- EMPRESAS: Add UUID as unique identifier (keep cnpj_basico as primary key)
-- Since empresas is partitioned by cnpj_basico, we create a unique index including cnpj_basico
ALTER TABLE empresas 
ADD COLUMN id UUID DEFAULT uuidv7();

-- Create unique index including partitioning column for partitioned tables
CREATE UNIQUE INDEX idx_empresas_uuid_unique ON empresas(id, cnpj_basico);
CREATE INDEX idx_empresas_uuid ON empresas(id);

-- ESTABELECIMENTOS: Add UUID as unique identifier
-- Since estabelecimentos is partitioned by cnpj_basico, include it in unique index
ALTER TABLE estabelecimentos 
ADD COLUMN uuid_id UUID DEFAULT uuidv7();

CREATE UNIQUE INDEX idx_estabelecimentos_uuid_unique ON estabelecimentos(uuid_id, cnpj_basico);
CREATE INDEX idx_estabelecimentos_uuid ON estabelecimentos(uuid_id);

-- SOCIOS: Add UUID as unique identifier
-- Since socios is partitioned by cnpj_basico, include it in unique index
ALTER TABLE socios 
ADD COLUMN uuid_id UUID DEFAULT uuidv7();

CREATE UNIQUE INDEX idx_socios_uuid_unique ON socios(uuid_id, cnpj_basico);
CREATE INDEX idx_socios_uuid ON socios(uuid_id);

-- SIMPLES: Add UUID as unique identifier
-- simples is NOT partitioned, so we can use UNIQUE constraint normally
ALTER TABLE simples 
ADD COLUMN uuid_id UUID UNIQUE DEFAULT uuidv7();

CREATE INDEX idx_simples_uuid ON simples(uuid_id);
