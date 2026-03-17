-- Create SÓCIOS table with partitioning
CREATE TABLE socios (
    id BIGSERIAL,
    cnpj_basico VARCHAR(8) NOT NULL REFERENCES empresas(cnpj_basico),
    identificador_socio VARCHAR(1),
    nome_socio VARCHAR(255) NOT NULL,
    cpf_cnpj_socio VARCHAR(14), -- Parcialmente mascarado
    qualificacao_socio VARCHAR(2) REFERENCES qualificacoes(codigo),
    data_entrada_sociedade DATE,
    pais VARCHAR(3) REFERENCES paises(codigo),
    representante_legal VARCHAR(14),
    nome_representante VARCHAR(255),
    qualificacao_representante VARCHAR(2),
    faixa_etaria VARCHAR(1),
    created_at TIMESTAMP DEFAULT NOW(),
    PRIMARY KEY (cnpj_basico, id)
) PARTITION BY RANGE (cnpj_basico);

-- Create partitions (10 partitions)
CREATE TABLE socios_p0 PARTITION OF socios FOR VALUES FROM ('00000000') TO ('10000000');
CREATE TABLE socios_p1 PARTITION OF socios FOR VALUES FROM ('10000000') TO ('20000000');
CREATE TABLE socios_p2 PARTITION OF socios FOR VALUES FROM ('20000000') TO ('30000000');
CREATE TABLE socios_p3 PARTITION OF socios FOR VALUES FROM ('30000000') TO ('40000000');
CREATE TABLE socios_p4 PARTITION OF socios FOR VALUES FROM ('40000000') TO ('50000000');
CREATE TABLE socios_p5 PARTITION OF socios FOR VALUES FROM ('50000000') TO ('60000000');
CREATE TABLE socios_p6 PARTITION OF socios FOR VALUES FROM ('60000000') TO ('70000000');
CREATE TABLE socios_p7 PARTITION OF socios FOR VALUES FROM ('70000000') TO ('80000000');
CREATE TABLE socios_p8 PARTITION OF socios FOR VALUES FROM ('80000000') TO ('90000000');
CREATE TABLE socios_p9 PARTITION OF socios FOR VALUES FROM ('90000000') TO ('99999999');

-- Create indexes
CREATE INDEX idx_socios_cnpj_basico ON socios(cnpj_basico);
CREATE INDEX idx_socios_nome_gin ON socios USING gin(nome_socio gin_trgm_ops);
CREATE INDEX idx_socios_cpf ON socios(cpf_cnpj_socio) WHERE cpf_cnpj_socio IS NOT NULL;
