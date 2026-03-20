#!/usr/bin/env bash
set -euo pipefail

psql "${DATABASE_URL:?DATABASE_URL is required}" <<'SQL'
CREATE TABLE IF NOT EXISTS empresas (
  cnpj_basico char(8) PRIMARY KEY,
  razao_social text NOT NULL,
  natureza_juridica varchar(4) NOT NULL,
  qualificacao_responsavel varchar(2),
  capital_social numeric(20,2) NOT NULL,
  porte_empresa varchar(2),
  ente_federativo_responsavel text
);
SQL
