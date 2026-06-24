-- Create reference tables (lookup tables)

-- MOTIVOS
CREATE TABLE motivos (
    codigo VARCHAR(2) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

-- MUNICIPIOS
CREATE TABLE municipios (
    codigo VARCHAR(4) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL,
    uf VARCHAR(2)
);
CREATE INDEX idx_municipios_uf ON municipios(uf);

-- NATUREZAS
CREATE TABLE naturezas (
    codigo VARCHAR(4) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

-- PAISES
CREATE TABLE paises (
    codigo VARCHAR(3) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

-- QUALIFICACOES
CREATE TABLE qualificacoes (
    codigo VARCHAR(2) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

-- CNAES
CREATE TABLE cnaes (
    codigo VARCHAR(7) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL,
    secao VARCHAR(1),  -- Derived from code (first digit)
    divisao VARCHAR(2), -- Derived (first two digits)
    created_at TIMESTAMP DEFAULT NOW()
);

CREATE INDEX idx_cnaes_descricao_gin ON cnaes USING gin(descricao gin_trgm_ops);
CREATE INDEX idx_cnaes_divisao ON cnaes(divisao);
