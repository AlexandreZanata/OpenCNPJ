-- Search indexes for VPS restore (no FK constraints — import uses --skip-refs).
\set ON_ERROR_STOP on

-- Hot path: JOIN empresas ON cnpj_basico — PK is dropped during import; must recreate.
CREATE UNIQUE INDEX IF NOT EXISTS empresas_cnpj_basico_uidx ON empresas (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_empresas_razao_social_gin ON empresas USING gin(razao_social gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_empresas_natureza_juridica ON empresas(natureza_juridica);
CREATE INDEX IF NOT EXISTS idx_empresas_porte ON empresas(porte_empresa);
-- Hot path: public CNPJ lookup. Use UF-prefixed names — index names are schema-global,
-- so a leftover idx_estabelecimentos_cnpj_completo on estabelecimentos_legacy_range would
-- make CREATE INDEX IF NOT EXISTS skip and leave UF partitions without indexes (seq scan).
CREATE INDEX IF NOT EXISTS idx_estab_uf_cnpj_completo ON estabelecimentos (cnpj_completo);
CREATE INDEX IF NOT EXISTS idx_estab_uf_cnpj_basico ON estabelecimentos (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnae ON estabelecimentos(cnae_fiscal_principal);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_municipio ON estabelecimentos(municipio);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_uf ON estabelecimentos(uf);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_situacao ON estabelecimentos(situacao_cadastral);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_nome_fantasia_gin ON estabelecimentos USING gin(nome_fantasia gin_trgm_ops);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cep ON estabelecimentos(cep);
CREATE INDEX IF NOT EXISTS idx_estabelecimentos_cnae_uf_situacao ON estabelecimentos(cnae_fiscal_principal, uf, situacao_cadastral);
CREATE INDEX IF NOT EXISTS idx_socios_cnpj_basico ON socios(cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_socios_nome_gin ON socios USING gin(nome_socio gin_trgm_ops);
-- Hot path: GetSimplesByBasico — without this, lookup seq-scans ~49M rows and 500s.
CREATE UNIQUE INDEX IF NOT EXISTS simples_cnpj_basico_uidx ON simples (cnpj_basico);
CREATE INDEX IF NOT EXISTS idx_simples_opcao ON simples(opcao_simples) WHERE opcao_simples = 'S';
CREATE INDEX IF NOT EXISTS idx_simples_mei ON simples(opcao_mei) WHERE opcao_mei = 'S';

ANALYZE empresas;
ANALYZE estabelecimentos;
ANALYZE socios;
ANALYZE simples;
