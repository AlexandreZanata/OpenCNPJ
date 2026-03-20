-- Standardize UUIDv7 across all tables and migrate hot tables to HASH partitioning.
-- This migration keeps legacy business keys to preserve API compatibility while
-- making uuid_id the primary technical identifier for new ingestion flows.

-- ---------------------------------------------------------------------------
-- 1) Reference tables: add uuid_id + unique index
-- ---------------------------------------------------------------------------
ALTER TABLE motivos ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE municipios ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE naturezas ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE paises ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE qualificacoes ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE cnaes ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();

UPDATE motivos SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE municipios SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE naturezas SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE paises SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE qualificacoes SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE cnaes SET uuid_id = uuidv7() WHERE uuid_id IS NULL;

ALTER TABLE motivos ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE municipios ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE naturezas ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE paises ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE qualificacoes ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE cnaes ALTER COLUMN uuid_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_motivos_uuid_id ON motivos(uuid_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_municipios_uuid_id ON municipios(uuid_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_naturezas_uuid_id ON naturezas(uuid_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_paises_uuid_id ON paises(uuid_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_qualificacoes_uuid_id ON qualificacoes(uuid_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_cnaes_uuid_id ON cnaes(uuid_id);

-- ---------------------------------------------------------------------------
-- 2) Core tables: normalize uuid_id naming and constraints
-- ---------------------------------------------------------------------------
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_name = 'empresas' AND column_name = 'id'
    ) AND NOT EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_name = 'empresas' AND column_name = 'uuid_id'
    ) THEN
        ALTER TABLE empresas RENAME COLUMN id TO uuid_id;
    END IF;
END $$;

ALTER TABLE empresas ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE estabelecimentos ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE socios ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();
ALTER TABLE simples ADD COLUMN IF NOT EXISTS uuid_id UUID DEFAULT uuidv7();

UPDATE empresas SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE estabelecimentos SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE socios SET uuid_id = uuidv7() WHERE uuid_id IS NULL;
UPDATE simples SET uuid_id = uuidv7() WHERE uuid_id IS NULL;

ALTER TABLE empresas ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE estabelecimentos ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE socios ALTER COLUMN uuid_id SET NOT NULL;
ALTER TABLE simples ALTER COLUMN uuid_id SET NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_empresas_uuid_id_unique ON empresas(uuid_id, cnpj_basico);
CREATE UNIQUE INDEX IF NOT EXISTS idx_estabelecimentos_uuid_id_unique ON estabelecimentos(uuid_id, cnpj_basico);
CREATE UNIQUE INDEX IF NOT EXISTS idx_socios_uuid_id_unique ON socios(uuid_id, cnpj_basico);
CREATE UNIQUE INDEX IF NOT EXISTS idx_simples_uuid_id_unique ON simples(uuid_id);

-- Natural-key dedupe for socios so reruns do not fail.
CREATE UNIQUE INDEX IF NOT EXISTS idx_socios_dedupe_natural ON socios(
    cnpj_basico,
    COALESCE(nome_socio, ''),
    COALESCE(cpf_cnpj_socio, ''),
    COALESCE(qualificacao_socio, ''),
    COALESCE(data_entrada_sociedade, DATE '0001-01-01')
);

-- ---------------------------------------------------------------------------
-- 3) Repartition hot write tables to HASH(8) on cnpj_basico
-- ---------------------------------------------------------------------------
ALTER TABLE estabelecimentos RENAME TO estabelecimentos_legacy_range;
ALTER TABLE socios RENAME TO socios_legacy_range;

CREATE TABLE estabelecimentos (
    id BIGSERIAL,
    uuid_id UUID NOT NULL DEFAULT uuidv7(),
    cnpj_basico VARCHAR(8) NOT NULL REFERENCES empresas(cnpj_basico),
    cnpj_ordem VARCHAR(4) NOT NULL,
    cnpj_dv VARCHAR(2) NOT NULL,
    cnpj_completo VARCHAR(14) GENERATED ALWAYS AS (cnpj_basico || cnpj_ordem || cnpj_dv) STORED,
    identificador_matriz_filial VARCHAR(1),
    nome_fantasia VARCHAR(255),
    situacao_cadastral VARCHAR(2),
    data_situacao_cadastral DATE,
    motivo_situacao_cadastral VARCHAR(2) REFERENCES motivos(codigo),
    nome_cidade_exterior VARCHAR(255),
    pais VARCHAR(3) REFERENCES paises(codigo),
    data_inicio_atividade DATE,
    cnae_fiscal_principal VARCHAR(7) REFERENCES cnaes(codigo),
    cnae_fiscal_secundaria TEXT,
    tipo_logradouro VARCHAR(50),
    logradouro VARCHAR(255),
    numero VARCHAR(20),
    complemento VARCHAR(255),
    bairro VARCHAR(100),
    cep VARCHAR(8),
    uf VARCHAR(2),
    municipio VARCHAR(4) REFERENCES municipios(codigo),
    ddd_1 VARCHAR(4),
    telefone_1 VARCHAR(20),
    ddd_2 VARCHAR(4),
    telefone_2 VARCHAR(20),
    ddd_fax VARCHAR(4),
    fax VARCHAR(20),
    email VARCHAR(255),
    situacao_especial VARCHAR(100),
    data_situacao_especial DATE,
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (uuid_id, cnpj_basico),
    UNIQUE (cnpj_basico, cnpj_ordem, cnpj_dv)
) PARTITION BY HASH (cnpj_basico);

CREATE TABLE estabelecimentos_h0 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 0);
CREATE TABLE estabelecimentos_h1 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 1);
CREATE TABLE estabelecimentos_h2 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 2);
CREATE TABLE estabelecimentos_h3 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 3);
CREATE TABLE estabelecimentos_h4 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 4);
CREATE TABLE estabelecimentos_h5 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 5);
CREATE TABLE estabelecimentos_h6 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 6);
CREATE TABLE estabelecimentos_h7 PARTITION OF estabelecimentos FOR VALUES WITH (MODULUS 8, REMAINDER 7);

INSERT INTO estabelecimentos (
    id, uuid_id, cnpj_basico, cnpj_ordem, cnpj_dv, identificador_matriz_filial,
    nome_fantasia, situacao_cadastral, data_situacao_cadastral, motivo_situacao_cadastral,
    nome_cidade_exterior, pais, data_inicio_atividade, cnae_fiscal_principal,
    cnae_fiscal_secundaria, tipo_logradouro, logradouro, numero, complemento,
    bairro, cep, uf, municipio, ddd_1, telefone_1, ddd_2, telefone_2, ddd_fax,
    fax, email, situacao_especial, data_situacao_especial, created_at
)
SELECT
    id, uuid_id, cnpj_basico, cnpj_ordem, cnpj_dv, identificador_matriz_filial,
    nome_fantasia, situacao_cadastral, data_situacao_cadastral, motivo_situacao_cadastral,
    nome_cidade_exterior, pais, data_inicio_atividade, cnae_fiscal_principal,
    cnae_fiscal_secundaria, tipo_logradouro, logradouro, numero, complemento,
    bairro, cep, uf, municipio, ddd_1, telefone_1, ddd_2, telefone_2, ddd_fax,
    fax, email, situacao_especial, data_situacao_especial, created_at
FROM estabelecimentos_legacy_range;

SELECT setval(
    pg_get_serial_sequence('estabelecimentos', 'id'),
    COALESCE((SELECT MAX(id) FROM estabelecimentos), 1),
    true
);

CREATE INDEX idx_estab_v8_cnpj_completo ON estabelecimentos(cnpj_completo);
CREATE INDEX idx_estab_v8_cnpj_basico ON estabelecimentos(cnpj_basico);
CREATE INDEX idx_estab_v8_cnae ON estabelecimentos(cnae_fiscal_principal);
CREATE INDEX idx_estab_v8_municipio ON estabelecimentos(municipio);
CREATE INDEX idx_estab_v8_uf ON estabelecimentos(uf);
CREATE INDEX idx_estab_v8_situacao ON estabelecimentos(situacao_cadastral);
CREATE INDEX idx_estab_v8_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);
CREATE INDEX idx_estab_v8_cep ON estabelecimentos(cep);
CREATE INDEX idx_estab_v8_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);
CREATE UNIQUE INDEX idx_estab_v8_uuid_dedupe ON estabelecimentos(uuid_id, cnpj_basico);

CREATE TABLE socios (
    id BIGSERIAL,
    uuid_id UUID NOT NULL DEFAULT uuidv7(),
    cnpj_basico VARCHAR(8) NOT NULL REFERENCES empresas(cnpj_basico),
    identificador_socio VARCHAR(1),
    nome_socio VARCHAR(255) NOT NULL,
    cpf_cnpj_socio VARCHAR(14),
    qualificacao_socio VARCHAR(2) REFERENCES qualificacoes(codigo),
    data_entrada_sociedade DATE,
    pais VARCHAR(3) REFERENCES paises(codigo),
    representante_legal VARCHAR(14),
    nome_representante VARCHAR(255),
    qualificacao_representante VARCHAR(2),
    faixa_etaria VARCHAR(1),
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (uuid_id, cnpj_basico)
) PARTITION BY HASH (cnpj_basico);

CREATE TABLE socios_h0 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 0);
CREATE TABLE socios_h1 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 1);
CREATE TABLE socios_h2 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 2);
CREATE TABLE socios_h3 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 3);
CREATE TABLE socios_h4 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 4);
CREATE TABLE socios_h5 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 5);
CREATE TABLE socios_h6 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 6);
CREATE TABLE socios_h7 PARTITION OF socios FOR VALUES WITH (MODULUS 8, REMAINDER 7);

INSERT INTO socios (
    id, uuid_id, cnpj_basico, identificador_socio, nome_socio, cpf_cnpj_socio,
    qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
    nome_representante, qualificacao_representante, faixa_etaria, created_at
)
SELECT
    id, uuid_id, cnpj_basico, identificador_socio, nome_socio, cpf_cnpj_socio,
    qualificacao_socio, data_entrada_sociedade, pais, representante_legal,
    nome_representante, qualificacao_representante, faixa_etaria, created_at
FROM socios_legacy_range;

SELECT setval(
    pg_get_serial_sequence('socios', 'id'),
    COALESCE((SELECT MAX(id) FROM socios), 1),
    true
);

CREATE INDEX idx_socios_v8_cnpj_basico ON socios(cnpj_basico);
CREATE INDEX idx_socios_v8_nome_gin ON socios USING gin(nome_socio gin_trgm_ops);
CREATE INDEX idx_socios_v8_cpf ON socios(cpf_cnpj_socio) WHERE cpf_cnpj_socio IS NOT NULL;
CREATE UNIQUE INDEX idx_socios_v8_uuid_dedupe ON socios(uuid_id, cnpj_basico);
CREATE UNIQUE INDEX idx_socios_v8_dedupe_natural_hash ON socios(
    cnpj_basico,
    COALESCE(nome_socio, ''),
    COALESCE(cpf_cnpj_socio, ''),
    COALESCE(qualificacao_socio, ''),
    COALESCE(data_entrada_sociedade, DATE '0001-01-01')
);
