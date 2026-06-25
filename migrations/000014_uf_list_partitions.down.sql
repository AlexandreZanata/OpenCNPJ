-- Rollback Phase 6 — restore HASH(cnpj_basico) partitioning (plan 02 Phase 6 down).

DROP MATERIALIZED VIEW IF EXISTS mv_stats_estabelecimentos_by_cnae_uf;
DROP MATERIALIZED VIEW IF EXISTS mv_stats_estabelecimentos_by_cnae;
DROP MATERIALIZED VIEW IF EXISTS mv_stats_estabelecimentos_by_uf;

ALTER TABLE estabelecimentos RENAME TO estabelecimentos_legacy_uf;

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
    busca tsvector GENERATED ALWAYS AS (to_tsvector('portuguese', coalesce(nome_fantasia, ''))) STORED,
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
    bairro, cep, uf, municipio, ddd_1, telefone_1, ddd_2, telefone_2, ddd_fax,
    fax, email, situacao_especial, data_situacao_especial, created_at
FROM estabelecimentos_legacy_uf;

SET session_replication_role = DEFAULT;

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
CREATE INDEX idx_estab_nome_fantasia_ativas
    ON estabelecimentos USING GIN (nome_fantasia gin_trgm_ops)
    WHERE situacao_cadastral = '02';
CREATE INDEX idx_estab_cnae_uf_ativas
    ON estabelecimentos (cnae_fiscal_principal, uf)
    WHERE situacao_cadastral = '02';
CREATE INDEX idx_estabelecimentos_busca_fts ON estabelecimentos USING GIN (busca);

DROP TABLE estabelecimentos_legacy_uf;

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

SELECT refresh_estabelecimento_stats();
