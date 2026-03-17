-- Create ESTABELECIMENTOS table with partitioning
CREATE TABLE estabelecimentos (
    id BIGSERIAL,
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
    cnae_fiscal_secundaria TEXT, -- CSV de códigos separados por vírgula
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
    PRIMARY KEY (cnpj_basico, cnpj_ordem, cnpj_dv)
) PARTITION BY RANGE (cnpj_basico);

-- Create partitions (10 partitions)
CREATE TABLE estabelecimentos_p0 PARTITION OF estabelecimentos FOR VALUES FROM ('00000000') TO ('10000000');
CREATE TABLE estabelecimentos_p1 PARTITION OF estabelecimentos FOR VALUES FROM ('10000000') TO ('20000000');
CREATE TABLE estabelecimentos_p2 PARTITION OF estabelecimentos FOR VALUES FROM ('20000000') TO ('30000000');
CREATE TABLE estabelecimentos_p3 PARTITION OF estabelecimentos FOR VALUES FROM ('30000000') TO ('40000000');
CREATE TABLE estabelecimentos_p4 PARTITION OF estabelecimentos FOR VALUES FROM ('40000000') TO ('50000000');
CREATE TABLE estabelecimentos_p5 PARTITION OF estabelecimentos FOR VALUES FROM ('50000000') TO ('60000000');
CREATE TABLE estabelecimentos_p6 PARTITION OF estabelecimentos FOR VALUES FROM ('60000000') TO ('70000000');
CREATE TABLE estabelecimentos_p7 PARTITION OF estabelecimentos FOR VALUES FROM ('70000000') TO ('80000000');
CREATE TABLE estabelecimentos_p8 PARTITION OF estabelecimentos FOR VALUES FROM ('80000000') TO ('90000000');
CREATE TABLE estabelecimentos_p9 PARTITION OF estabelecimentos FOR VALUES FROM ('90000000') TO ('99999999');

-- Critical indexes for performance
-- Note: UNIQUE index on cnpj_completo cannot be created on partitioned table
-- Instead, we use the composite PRIMARY KEY (cnpj_basico, cnpj_ordem, cnpj_dv)
CREATE INDEX idx_estabelecimentos_cnpj_completo ON estabelecimentos(cnpj_completo);
CREATE INDEX idx_estabelecimentos_cnpj_basico ON estabelecimentos(cnpj_basico);
CREATE INDEX idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal);
CREATE INDEX idx_estabelecimentos_municipio ON estabelecimentos(municipio);
CREATE INDEX idx_estabelecimentos_uf ON estabelecimentos(uf);
CREATE INDEX idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral);
CREATE INDEX idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);
CREATE INDEX idx_estabelecimentos_cep ON estabelecimentos(cep);

-- Composite index for frequent queries
CREATE INDEX idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);
