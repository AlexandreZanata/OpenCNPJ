-- Speed up city typeahead (ILIKE on descricao).
CREATE INDEX IF NOT EXISTS idx_municipios_descricao_gin
    ON municipios USING gin (descricao gin_trgm_ops);
