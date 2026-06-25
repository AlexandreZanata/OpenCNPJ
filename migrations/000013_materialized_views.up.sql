-- Plan 02 Phase 4 — materialized views for analytics and lookup typeahead.
-- Replaces stats aggregate tables (000009) with REFRESH MATERIALIZED VIEW CONCURRENTLY.

CREATE MATERIALIZED VIEW mv_stats_estabelecimentos_by_uf AS
SELECT
    uf,
    COUNT(*)::BIGINT AS count,
    NOW() AS refreshed_at
FROM estabelecimentos
WHERE uf IS NOT NULL AND uf <> ''
GROUP BY uf
WITH NO DATA;

CREATE MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae AS
SELECT
    cnae_fiscal_principal AS cnae,
    COUNT(*)::BIGINT AS count,
    NOW() AS refreshed_at
FROM estabelecimentos
WHERE cnae_fiscal_principal IS NOT NULL
GROUP BY cnae_fiscal_principal
WITH NO DATA;

CREATE MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae_uf AS
SELECT
    cnae_fiscal_principal AS cnae,
    uf,
    COUNT(*)::BIGINT AS count,
    NOW() AS refreshed_at
FROM estabelecimentos
WHERE cnae_fiscal_principal IS NOT NULL AND uf IS NOT NULL
GROUP BY cnae_fiscal_principal, uf
WITH NO DATA;

CREATE MATERIALIZED VIEW mv_lookup_cnaes AS
SELECT codigo, descricao, secao, divisao
FROM cnaes;

CREATE MATERIALIZED VIEW mv_lookup_municipios AS
SELECT codigo, descricao, COALESCE(uf, '') AS uf
FROM municipios;

CREATE UNIQUE INDEX uidx_mv_stats_uf ON mv_stats_estabelecimentos_by_uf (uf);
CREATE UNIQUE INDEX uidx_mv_stats_cnae ON mv_stats_estabelecimentos_by_cnae (cnae);
CREATE UNIQUE INDEX uidx_mv_stats_cnae_uf ON mv_stats_estabelecimentos_by_cnae_uf (cnae, uf);
CREATE UNIQUE INDEX uidx_mv_lookup_cnaes ON mv_lookup_cnaes (codigo);
CREATE UNIQUE INDEX uidx_mv_lookup_municipios ON mv_lookup_municipios (codigo);

CREATE INDEX idx_mv_stats_cnae_count ON mv_stats_estabelecimentos_by_cnae (count DESC);
CREATE INDEX idx_mv_stats_cnae_uf_cnae ON mv_stats_estabelecimentos_by_cnae_uf (cnae);
CREATE INDEX idx_mv_lookup_municipios_descricao_gin
    ON mv_lookup_municipios USING gin (descricao gin_trgm_ops);
CREATE INDEX idx_mv_lookup_cnaes_descricao_gin
    ON mv_lookup_cnaes USING gin (descricao gin_trgm_ops);

-- Populate stats MVs from legacy tables when present (instant); else run refresh_estabelecimento_stats().
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM stats_estabelecimentos_by_uf LIMIT 1) THEN
        REFRESH MATERIALIZED VIEW mv_stats_estabelecimentos_by_uf;
        REFRESH MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae;
        REFRESH MATERIALIZED VIEW mv_stats_estabelecimentos_by_cnae_uf;
    END IF;
END $$;

DROP FUNCTION IF EXISTS refresh_estabelecimento_stats();
DROP TABLE IF EXISTS stats_estabelecimentos_by_cnae_uf;
DROP TABLE IF EXISTS stats_estabelecimentos_by_cnae;
DROP TABLE IF EXISTS stats_estabelecimentos_by_uf;

CREATE OR REPLACE FUNCTION refresh_estabelecimento_stats()
RETURNS TABLE(
    uf_rows INT,
    cnae_rows INT,
    cnae_uf_rows INT,
    lookup_cnae_rows INT,
    lookup_municipio_rows INT
) AS $$
DECLARE
    v_uf INT;
    v_cnae INT;
    v_cnae_uf INT;
    v_lc INT;
    v_lm INT;
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_stats_estabelecimentos_by_uf;
    SELECT COUNT(*)::INT INTO v_uf FROM mv_stats_estabelecimentos_by_uf;

    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_stats_estabelecimentos_by_cnae;
    SELECT COUNT(*)::INT INTO v_cnae FROM mv_stats_estabelecimentos_by_cnae;

    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_stats_estabelecimentos_by_cnae_uf;
    SELECT COUNT(*)::INT INTO v_cnae_uf FROM mv_stats_estabelecimentos_by_cnae_uf;

    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_lookup_cnaes;
    SELECT COUNT(*)::INT INTO v_lc FROM mv_lookup_cnaes;

    REFRESH MATERIALIZED VIEW CONCURRENTLY mv_lookup_municipios;
    SELECT COUNT(*)::INT INTO v_lm FROM mv_lookup_municipios;

    RETURN QUERY SELECT v_uf, v_cnae, v_cnae_uf, v_lc, v_lm;
END;
$$ LANGUAGE plpgsql;
