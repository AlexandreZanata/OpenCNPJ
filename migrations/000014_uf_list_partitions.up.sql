-- Plan 02 Phase 6 — LIST(uf) partitioning for UF-filtered search pruning.
-- Swaps HASH(cnpj_basico) write layout for LIST(uf) read pruning; off-peak window required on VPS.
-- Rollback: migrations/000014_uf_list_partitions.down.sql

DROP MATERIALIZED VIEW IF EXISTS mv_stats_estabelecimentos_by_cnae_uf;
DROP MATERIALIZED VIEW IF EXISTS mv_stats_estabelecimentos_by_cnae;
DROP MATERIALIZED VIEW IF EXISTS mv_stats_estabelecimentos_by_uf;

ALTER TABLE estabelecimentos RENAME TO estabelecimentos_legacy_hash;

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
    motivo_situacao_cadastral VARCHAR(2),
    nome_cidade_exterior VARCHAR(255),
    pais VARCHAR(3),
    data_inicio_atividade DATE,
    cnae_fiscal_principal VARCHAR(7),
    cnae_fiscal_secundaria TEXT,
    tipo_logradouro VARCHAR(50),
    logradouro VARCHAR(255),
    numero VARCHAR(20),
    complemento VARCHAR(255),
    bairro VARCHAR(100),
    cep VARCHAR(8),
    uf VARCHAR(2) NOT NULL,
    municipio VARCHAR(4),
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
    busca tsvector GENERATED ALWAYS AS (to_tsvector('portuguese', coalesce(nome_fantasia, ''))) STORED,
    PRIMARY KEY (uuid_id, cnpj_basico, uf),
    UNIQUE (cnpj_basico, cnpj_ordem, cnpj_dv, uf)
) PARTITION BY LIST (uf);

DO $$
DECLARE
    uf_code TEXT;
    ufs TEXT[] := ARRAY[
        'AC','AL','AP','AM','BA','CE','DF','ES','GO','MA','MT','MS','MG',
        'PA','PB','PR','PE','PI','RJ','RN','RS','RO','RR','SC','SP','SE','TO','EX'
    ];
BEGIN
    FOREACH uf_code IN ARRAY ufs LOOP
        EXECUTE format(
            'CREATE TABLE estabelecimentos_%s PARTITION OF estabelecimentos FOR VALUES IN (%L)',
            lower(uf_code), uf_code
        );
    END LOOP;
    EXECUTE 'CREATE TABLE estabelecimentos_default PARTITION OF estabelecimentos DEFAULT';
END $$;

SET session_replication_role = replica;

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
    bairro, cep, COALESCE(NULLIF(TRIM(uf), ''), 'EX'), municipio, ddd_1, telefone_1,
    ddd_2, telefone_2, ddd_fax, fax, email, situacao_especial, data_situacao_especial,
    created_at
FROM estabelecimentos_legacy_hash;

SET session_replication_role = DEFAULT;

SELECT setval(
    pg_get_serial_sequence('estabelecimentos', 'id'),
    COALESCE((SELECT MAX(id) FROM estabelecimentos), 1),
    true
);

CREATE INDEX IF NOT EXISTS idx_estab_uf_cnpj_completo ON estabelecimentos(cnpj_completo);
CREATE INDEX IF NOT EXISTS idx_estab_uf_cnpj_basico ON estabelecimentos(cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_estab_uf_cnae ON estabelecimentos(cnae_fiscal_principal);
CREATE INDEX IF NOT EXISTS idx_estab_uf_municipio ON estabelecimentos(municipio);
CREATE INDEX IF NOT EXISTS idx_estab_uf_situacao ON estabelecimentos(situacao_cadastral);
CREATE INDEX IF NOT EXISTS idx_estab_uf_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_estab_uf_cep ON estabelecimentos(cep);
CREATE INDEX IF NOT EXISTS idx_estab_uf_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);
CREATE UNIQUE INDEX IF NOT EXISTS idx_estab_uf_uuid_dedupe ON estabelecimentos(uuid_id, cnpj_basico, uf);
CREATE INDEX IF NOT EXISTS idx_estab_uf_nome_fantasia_ativas
    ON estabelecimentos USING GIN (nome_fantasia gin_trgm_ops)
    WHERE situacao_cadastral = '02';
CREATE INDEX IF NOT EXISTS idx_estab_uf_cnae_uf_ativas
    ON estabelecimentos (cnae_fiscal_principal, uf)
    WHERE situacao_cadastral = '02';
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_busca_fts ON estabelecimentos USING GIN (busca);

DROP TABLE estabelecimentos_legacy_hash;

CREATE MATERIALIZED VIEW mv_stats_estabelecimentos_by_uf AS
SELECT uf, COUNT(*)::BIGINT AS count, NOW() AS refreshed_at
FROM estabelecimentos
WHERE uf IS NOT NULL AND uf <> ''
GROUP BY uf
WITH NO DATA;

CREATE MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae AS
SELECT cnae_fiscal_principal AS cnae, COUNT(*)::BIGINT AS count, NOW() AS refreshed_at
FROM estabelecimentos
WHERE cnae_fiscal_principal IS NOT NULL
GROUP BY cnae_fiscal_principal
WITH NO DATA;

CREATE MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae_uf AS
SELECT cnae_fiscal_principal AS cnae, uf, COUNT(*)::BIGINT AS count, NOW() AS refreshed_at
FROM estabelecimentos
WHERE cnae_fiscal_principal IS NOT NULL AND uf IS NOT NULL
GROUP BY cnae_fiscal_principal, uf
WITH NO DATA;

CREATE UNIQUE INDEX uidx_mv_stats_uf ON mv_stats_estabelecimentos_by_uf (uf);
CREATE UNIQUE INDEX uidx_mv_stats_cnae ON mv_stats_estabelecimentos_by_cnae (cnae);
CREATE UNIQUE INDEX uidx_mv_stats_cnae_uf ON mv_stats_estabelecimentos_by_cnae_uf (cnae, uf);
CREATE INDEX idx_mv_stats_cnae_count ON mv_stats_estabelecimentos_by_cnae (count DESC);
CREATE INDEX idx_mv_stats_cnae_uf_cnae ON mv_stats_estabelecimentos_by_cnae_uf (cnae);

REFRESH MATERIALIZED VIEW mv_stats_estabelecimentos_by_uf;
REFRESH MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae;
REFRESH MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae_uf;
