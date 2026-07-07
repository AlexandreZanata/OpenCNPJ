-- sqlc schema snapshot (logical tables; production uses partitions from migrations/)

CREATE TABLE motivos (
    codigo VARCHAR(2) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

CREATE TABLE municipios (
    codigo VARCHAR(4) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL,
    uf VARCHAR(2)
);

CREATE TABLE naturezas (
    codigo VARCHAR(4) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

CREATE TABLE paises (
    codigo VARCHAR(3) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

CREATE TABLE qualificacoes (
    codigo VARCHAR(2) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

CREATE TABLE cnaes (
    codigo VARCHAR(7) PRIMARY KEY,
    descricao VARCHAR(255) NOT NULL
);

CREATE TABLE empresas (
    uuid_id UUID NOT NULL,
    cnpj_basico VARCHAR(8) PRIMARY KEY,
    razao_social VARCHAR(255) NOT NULL,
    natureza_juridica VARCHAR(4) REFERENCES naturezas (codigo),
    qualificacao_responsavel VARCHAR(2) REFERENCES qualificacoes (codigo),
    capital_social DECIMAL(15, 2),
    porte_empresa VARCHAR(2),
    ente_federativo_responsavel VARCHAR(255),
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);

CREATE TABLE estabelecimentos (
    id BIGSERIAL,
    uuid_id UUID NOT NULL,
    cnpj_basico VARCHAR(8) NOT NULL REFERENCES empresas (cnpj_basico),
    cnpj_ordem VARCHAR(4) NOT NULL,
    cnpj_dv VARCHAR(2) NOT NULL,
    cnpj_completo VARCHAR(14) GENERATED ALWAYS AS (cnpj_basico || cnpj_ordem || cnpj_dv) STORED,
    identificador_matriz_filial VARCHAR(1),
    nome_fantasia VARCHAR(255),
    situacao_cadastral VARCHAR(2),
    data_situacao_cadastral DATE,
    motivo_situacao_cadastral VARCHAR(2) REFERENCES motivos (codigo),
    nome_cidade_exterior VARCHAR(255),
    pais VARCHAR(3) REFERENCES paises (codigo),
    data_inicio_atividade DATE,
    cnae_fiscal_principal VARCHAR(7) REFERENCES cnaes (codigo),
    cnae_fiscal_secundaria TEXT,
    tipo_logradouro VARCHAR(50),
    logradouro VARCHAR(255),
    numero VARCHAR(20),
    complemento VARCHAR(255),
    bairro VARCHAR(100),
    cep VARCHAR(8),
    uf VARCHAR(2),
    municipio VARCHAR(4) REFERENCES municipios (codigo),
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
);

CREATE INDEX idx_estabelecimentos_cnpj_completo ON estabelecimentos (cnpj_completo);

CREATE TABLE socios (
    id BIGSERIAL,
    uuid_id UUID NOT NULL,
    cnpj_basico VARCHAR(8) NOT NULL REFERENCES empresas (cnpj_basico),
    identificador_socio VARCHAR(1),
    nome_socio VARCHAR(255) NOT NULL,
    cpf_cnpj_socio VARCHAR(14),
    qualificacao_socio VARCHAR(2) REFERENCES qualificacoes (codigo),
    data_entrada_sociedade DATE,
    pais VARCHAR(3) REFERENCES paises (codigo),
    representante_legal VARCHAR(14),
    nome_representante VARCHAR(255),
    qualificacao_representante VARCHAR(2),
    faixa_etaria VARCHAR(1),
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (cnpj_basico, id)
);

CREATE INDEX idx_socios_cnpj_basico ON socios (cnpj_basico);

CREATE TABLE simples (
    uuid_id UUID NOT NULL,
    cnpj_basico VARCHAR(8) PRIMARY KEY REFERENCES empresas (cnpj_basico),
    opcao_simples CHAR(1),
    data_opcao_simples DATE,
    data_exclusao_simples DATE,
    opcao_mei CHAR(1),
    data_opcao_mei DATE,
    data_exclusao_mei DATE
);
