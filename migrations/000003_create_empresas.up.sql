-- Create EMPRESAS table with partitioning
CREATE TABLE empresas (
    cnpj_basico VARCHAR(8) PRIMARY KEY,
    razao_social VARCHAR(255) NOT NULL,
    natureza_juridica VARCHAR(4) REFERENCES naturezas(codigo),
    qualificacao_responsavel VARCHAR(2) REFERENCES qualificacoes(codigo),
    capital_social DECIMAL(15,2),
    porte_empresa VARCHAR(2),
    ente_federativo_responsavel VARCHAR(255),
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
) PARTITION BY RANGE (cnpj_basico);

-- Create partitions (10 partitions covering 00000000-99999999)
CREATE TABLE empresas_p0 PARTITION OF empresas FOR VALUES FROM ('00000000') TO ('10000000');
CREATE TABLE empresas_p1 PARTITION OF empresas FOR VALUES FROM ('10000000') TO ('20000000');
CREATE TABLE empresas_p2 PARTITION OF empresas FOR VALUES FROM ('20000000') TO ('30000000');
CREATE TABLE empresas_p3 PARTITION OF empresas FOR VALUES FROM ('30000000') TO ('40000000');
CREATE TABLE empresas_p4 PARTITION OF empresas FOR VALUES FROM ('40000000') TO ('50000000');
CREATE TABLE empresas_p5 PARTITION OF empresas FOR VALUES FROM ('50000000') TO ('60000000');
CREATE TABLE empresas_p6 PARTITION OF empresas FOR VALUES FROM ('60000000') TO ('70000000');
CREATE TABLE empresas_p7 PARTITION OF empresas FOR VALUES FROM ('70000000') TO ('80000000');
CREATE TABLE empresas_p8 PARTITION OF empresas FOR VALUES FROM ('80000000') TO ('90000000');
CREATE TABLE empresas_p9 PARTITION OF empresas FOR VALUES FROM ('90000000') TO ('99999999');

-- Create indexes
CREATE INDEX idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops);
CREATE INDEX idx_empresas_natureza ON empresas(natureza_juridica);
CREATE INDEX idx_empresas_porte ON empresas(porte_empresa);
