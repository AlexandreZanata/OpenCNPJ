-- Create SIMPLES table
CREATE TABLE simples (
    cnpj_basico VARCHAR(8) PRIMARY KEY REFERENCES empresas(cnpj_basico),
    opcao_simples CHAR(1), -- S/N
    data_opcao_simples DATE,
    data_exclusao_simples DATE,
    opcao_mei CHAR(1), -- S/N
    data_opcao_mei DATE,
    data_exclusao_mei DATE
);

-- Create partial indexes
CREATE INDEX idx_simples_opcao ON simples(opcao_simples) WHERE opcao_simples = 'S';
CREATE INDEX idx_simples_mei ON simples(opcao_mei) WHERE opcao_mei = 'S';
