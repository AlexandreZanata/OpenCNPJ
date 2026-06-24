-- Partial indexes for active estabelecimentos (situacao_cadastral = '02').
-- pg_trgm similarity_threshold tuned via postgresql.conf (default 0.45).
CREATE INDEX IF NOT EXISTS idx_estab_nome_fantasia_ativas
    ON estabelecimentos USING GIN (nome_fantasia gin_trgm_ops)
    WHERE situacao_cadastral = '02';

CREATE INDEX IF NOT EXISTS idx_estab_cnae_uf_ativas
    ON estabelecimentos (cnae_fiscal_principal, uf)
    WHERE situacao_cadastral = '02';
