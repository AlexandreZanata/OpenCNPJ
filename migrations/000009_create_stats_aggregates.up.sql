-- Pre-aggregated statistics for fast analytics API reads (<30s SLA).
-- Refresh after full import: SELECT refresh_estabelecimento_stats();

CREATE TABLE stats_estabelecimentos_by_uf (
    uf VARCHAR(2) PRIMARY KEY,
    count BIGINT NOT NULL CHECK (count >= 0),
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE stats_estabelecimentos_by_cnae (
    cnae VARCHAR(7) PRIMARY KEY,
    count BIGINT NOT NULL CHECK (count >= 0),
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_stats_estab_cnae_count ON stats_estabelecimentos_by_cnae (count DESC);

CREATE TABLE stats_estabelecimentos_by_cnae_uf (
    cnae VARCHAR(7) NOT NULL,
    uf VARCHAR(2) NOT NULL,
    count BIGINT NOT NULL CHECK (count >= 0),
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (cnae, uf)
);

CREATE INDEX idx_stats_estab_cnae_uf_cnae ON stats_estabelecimentos_by_cnae_uf (cnae);
CREATE INDEX idx_stats_estab_cnae_uf_count ON stats_estabelecimentos_by_cnae_uf (cnae, count DESC);

CREATE OR REPLACE FUNCTION refresh_estabelecimento_stats()
RETURNS TABLE(uf_rows INT, cnae_rows INT, cnae_uf_rows INT) AS $$
DECLARE
    v_uf INT;
    v_cnae INT;
    v_cnae_uf INT;
BEGIN
    TRUNCATE stats_estabelecimentos_by_uf;
    INSERT INTO stats_estabelecimentos_by_uf (uf, count, refreshed_at)
    SELECT uf, COUNT(*), NOW()
    FROM estabelecimentos
    WHERE uf IS NOT NULL
    GROUP BY uf;
    GET DIAGNOSTICS v_uf = ROW_COUNT;

    TRUNCATE stats_estabelecimentos_by_cnae;
    INSERT INTO stats_estabelecimentos_by_cnae (cnae, count, refreshed_at)
    SELECT cnae_fiscal_principal, COUNT(*), NOW()
    FROM estabelecimentos
    WHERE cnae_fiscal_principal IS NOT NULL
    GROUP BY cnae_fiscal_principal;
    GET DIAGNOSTICS v_cnae = ROW_COUNT;

    TRUNCATE stats_estabelecimentos_by_cnae_uf;
    INSERT INTO stats_estabelecimentos_by_cnae_uf (cnae, uf, count, refreshed_at)
    SELECT cnae_fiscal_principal, uf, COUNT(*), NOW()
    FROM estabelecimentos
    WHERE cnae_fiscal_principal IS NOT NULL AND uf IS NOT NULL
    GROUP BY cnae_fiscal_principal, uf;
    GET DIAGNOSTICS v_cnae_uf = ROW_COUNT;

    RETURN QUERY SELECT v_uf, v_cnae, v_cnae_uf;
END;
$$ LANGUAGE plpgsql;
