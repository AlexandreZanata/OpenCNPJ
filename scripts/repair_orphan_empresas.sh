#!/usr/bin/env bash
# Repair missing empresas rows from EMPRECSV into local Postgres (Docker).
# Prerequisite: non-truncated EMPRECSV files under ./data (see downloader maxZipMemberBytes fix).
#
# Usage:
#   ./scripts/repair_orphan_empresas.sh
#   DATABASE_URL=postgres://... ./scripts/repair_orphan_empresas.sh   # optional direct DSN
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DATA="${DATA_PATH:-$ROOT/data}"
PG_CONTAINER="${PG_CONTAINER:-receita-postgres}"
PG_USER="${PG_USER:-receita_user}"
PG_DB="${PG_DB:-receita_db}"

log() { echo "[$(date '+%H:%M:%S')] $*"; }

before_orphans() {
  sg docker -c "docker exec $PG_CONTAINER psql -U $PG_USER -d $PG_DB -At -c \"
    SELECT count(*) FROM estabelecimentos e
    WHERE NOT EXISTS (SELECT 1 FROM empresas emp WHERE emp.cnpj_basico = e.cnpj_basico);
  \""
}

log "orphan estabelecimentos before: $(before_orphans)"

# Stage EMPRECSV via temp table then insert missing only (idempotent).
sg docker -c "docker exec -i $PG_CONTAINER psql -U $PG_USER -d $PG_DB" <<'SQL'
CREATE TABLE IF NOT EXISTS empresas_repair_stage (
  cnpj_basico VARCHAR(8) NOT NULL,
  razao_social VARCHAR(255) NOT NULL,
  natureza_juridica VARCHAR(4),
  qualificacao_responsavel VARCHAR(2),
  capital_social NUMERIC,
  porte_empresa VARCHAR(2),
  ente_federativo_responsavel VARCHAR(255)
);
TRUNCATE empresas_repair_stage;
SQL

shopt -s nullglob
files=("$DATA"/K3241*.EMPRECSV)
if [[ ${#files[@]} -eq 0 ]]; then
  echo "ERROR: no EMPRECSV under $DATA" >&2
  exit 1
fi

for f in "${files[@]}"; do
  size=$(stat -c%s "$f")
  if [[ "$size" -eq $((512 * 1024 * 1024)) ]]; then
    echo "ERROR: $f is exactly 512MiB (likely truncated). Re-download/extract first." >&2
    exit 1
  fi
  log "staging $(basename "$f") ($size bytes)"
  python3 - "$f" <<'PY' | sg docker -c "docker exec -i $PG_CONTAINER psql -U $PG_USER -d $PG_DB -c \"COPY empresas_repair_stage (cnpj_basico, razao_social, natureza_juridica, qualificacao_responsavel, capital_social, porte_empresa, ente_federativo_responsavel) FROM STDIN WITH (FORMAT csv, DELIMITER ';', QUOTE '\\\"', ENCODING 'LATIN1')\""
import csv, sys
from pathlib import Path
path = Path(sys.argv[1])
seen = set()
with path.open("r", encoding="latin-1", newline="") as fh:
    reader = csv.reader(fh, delimiter=";")
    w = csv.writer(sys.stdout, delimiter=";", quoting=csv.QUOTE_MINIMAL, lineterminator="\n")
    for row in reader:
        if len(row) < 7:
            continue
        basico = row[0].strip()
        if len(basico) != 8 or not basico.isdigit() or basico in seen:
            continue
        seen.add(basico)
        razao = (row[1] or "").strip() or "RAZAO NAO INFORMADA"
        natureza = row[2].strip() or ""
        qual = row[3].strip() or ""
        capital_raw = (row[4] or "0").replace(".", "").replace(",", ".")
        porte = row[5].strip() or ""
        ente = row[6].strip() or ""
        w.writerow([basico, razao[:255], natureza, qual, capital_raw, porte, ente])
PY
done

log "inserting missing orphan empresas from stage (drop GIN for speed)"
sg docker -c "docker exec -i $PG_CONTAINER psql -U $PG_USER -d $PG_DB" <<'SQL'
DROP INDEX IF EXISTS idx_empresas_razao_social_gin;
CREATE TEMP TABLE orphan_basicos AS
SELECT DISTINCT e.cnpj_basico
FROM estabelecimentos e
WHERE NOT EXISTS (SELECT 1 FROM empresas emp WHERE emp.cnpj_basico = e.cnpj_basico);

INSERT INTO empresas (
  cnpj_basico, razao_social, natureza_juridica, qualificacao_responsavel,
  capital_social, porte_empresa, ente_federativo_responsavel
)
SELECT DISTINCT ON (s.cnpj_basico)
  s.cnpj_basico,
  s.razao_social,
  CASE WHEN EXISTS (SELECT 1 FROM naturezas n WHERE n.codigo = s.natureza_juridica)
       THEN s.natureza_juridica END,
  CASE WHEN EXISTS (SELECT 1 FROM qualificacoes q WHERE q.codigo = s.qualificacao_responsavel)
       THEN s.qualificacao_responsavel END,
  s.capital_social,
  s.porte_empresa,
  s.ente_federativo_responsavel
FROM orphan_basicos o
JOIN empresas_repair_stage s ON s.cnpj_basico = o.cnpj_basico
ORDER BY s.cnpj_basico
ON CONFLICT (cnpj_basico) DO NOTHING;

INSERT INTO empresas (cnpj_basico, razao_social)
SELECT o.cnpj_basico,
       COALESCE(NULLIF(MAX(e.nome_fantasia), ''), 'RAZAO AUSENTE NO EMPRECSV')
FROM orphan_basicos o
JOIN estabelecimentos e ON e.cnpj_basico = o.cnpj_basico
WHERE NOT EXISTS (SELECT 1 FROM empresas emp WHERE emp.cnpj_basico = o.cnpj_basico)
GROUP BY o.cnpj_basico
ON CONFLICT (cnpj_basico) DO NOTHING;

CREATE INDEX IF NOT EXISTS idx_empresas_razao_social_gin ON empresas USING gin (razao_social gin_trgm_ops);
SQL

log "orphan estabelecimentos after: $(before_orphans)"
log "AMAGGI check:"
sg docker -c "docker exec $PG_CONTAINER psql -U $PG_USER -d $PG_DB -c \"
  SELECT cnpj_basico, left(razao_social,60) FROM empresas WHERE cnpj_basico='77294254';
\""
log "repair complete"
