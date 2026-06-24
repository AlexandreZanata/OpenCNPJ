-- Full-text search columns (portuguese dictionary) for multi-word queries.
ALTER TABLE empresas
    ADD COLUMN IF NOT EXISTS busca tsvector
    GENERATED ALWAYS AS (to_tsvector('portuguese', coalesce(razao_social, ''))) STORED;

ALTER TABLE estabelecimentos
    ADD COLUMN IF NOT EXISTS busca tsvector
    GENERATED ALWAYS AS (to_tsvector('portuguese', coalesce(nome_fantasia, ''))) STORED;

CREATE INDEX IF NOT EXISTS idx_empresas_busca_fts
    ON empresas USING GIN (busca);

CREATE INDEX IF NOT EXISTS idx_estabelecimentos_busca_fts
    ON estabelecimentos USING GIN (busca);
