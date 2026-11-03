#!/usr/bin/env bash
# Builds comparison baselines from the same official ZIPs and records sizes and timings:
#   csv/      extracted CSV members (B)
#   parquet/  one Parquet file per table, ZSTD (C), via DuckDB
#   cnpj.sqlite  SQLite with B-tree indexes for the API lookups/filters and FTS5 over names (D)
#
# Usage: scripts/baselines.sh RAW_DIR OUT_DIR [zstd_level]
# Needs: unzip, duckdb (>= 1.1), python3 whose sqlite3 module has FTS5. Writes OUT_DIR/baselines.json.
set -euo pipefail

RAW=${1:?usage: baselines.sh RAW_DIR OUT_DIR [zstd_level]}
OUT=${2:?usage: baselines.sh RAW_DIR OUT_DIR [zstd_level]}
LEVEL=${3:-9}
mkdir -p "$OUT/csv" "$OUT/parquet"
now() { date +%s.%N; }
results="$OUT/baselines.tsv"
: > "$results"
record() { printf '%s\t%s\t%s\n' "$1" "$2" "$3" >> "$results"; }

# B. Extracted CSV (SKIP_EXTRACT=1 reuses OUT/csv)
if [ "${SKIP_EXTRACT:-0}" != 1 ]; then
  t0=$(now)
  for z in "$RAW"/*.zip; do
    name=$(basename "$z" .zip)
    unzip -p "$z" > "$OUT/csv/$name.csv"
  done
  record csv_extract_s "$(echo "$(now) - $t0" | bc)" ""
fi
record csv_bytes "$(du -cb "$OUT"/csv/*.csv | tail -1 | cut -f1)" ""

# DuckDB's latin-1 reader rejects some valid ISO-8859-1 bytes; transcode with iconv.
mkdir -p "$OUT/csv-utf8"
t0=$(now)
for f in "$OUT"/csv/*.csv; do
  iconv -f ISO-8859-1 -t UTF-8 "$f" > "$OUT/csv-utf8/$(basename "$f")"
done
record csv_to_utf8_s "$(echo "$(now) - $t0" | bc)" "iconv, needed by DuckDB"

cols_empresas="cnpj_basico,razao_social,natureza_juridica,qualificacao_responsavel,capital_social,porte_empresa,ente_federativo_responsavel"
cols_estabelecimentos="cnpj_basico,cnpj_ordem,cnpj_dv,identificador_matriz_filial,nome_fantasia,situacao_cadastral,data_situacao_cadastral,motivo_situacao_cadastral,nome_cidade_exterior,pais,data_inicio_atividade,cnae_fiscal_principal,cnae_fiscal_secundaria,tipo_logradouro,logradouro,numero,complemento,bairro,cep,uf,municipio,ddd_1,telefone_1,ddd_2,telefone_2,ddd_fax,fax,correio_eletronico,situacao_especial,data_situacao_especial"
cols_socios="cnpj_basico,identificador_socio,nome_socio_razao_social,cpf_cnpj_socio,qualificacao_socio,data_entrada_sociedade,pais,representante_legal,nome_do_representante,qualificacao_representante_legal,faixa_etaria"
cols_simples="cnpj_basico,opcao_pelo_simples,data_opcao_simples,data_exclusao_simples,opcao_mei,data_opcao_mei,data_exclusao_mei"

colspec() { # "a,b,c" -> {'a':'VARCHAR','b':'VARCHAR',...}
  echo "{$(echo "$1" | tr ',' '\n' | sed "s/.*/'&':'VARCHAR'/" | paste -sd,)}"
}

# C. Parquet + ZSTD (all columns as text: same semantic content, no type coercion).
# DuckDB's parallel CSV reader rejects these files (unescaped quotes), hence parallel=false.
t0=$(now)
for t in empresas estabelecimentos socios simples; do
  case $t in
    empresas) glob="Empresas*.csv"; cols=$cols_empresas ;;
    estabelecimentos) glob="Estabelecimentos*.csv"; cols=$cols_estabelecimentos ;;
    socios) glob="Socios*.csv"; cols=$cols_socios ;;
    simples) glob="Simples.csv"; cols=$cols_simples ;;
  esac
  duckdb -c "SET threads=$(nproc); COPY (SELECT * FROM read_csv('$OUT/csv-utf8/$glob', delim=';', quote='\"', escape='', header=false, strict_mode=false, parallel=false, columns=$(colspec "$cols"))) TO '$OUT/parquet/$t.parquet' (FORMAT parquet, COMPRESSION zstd, COMPRESSION_LEVEL $LEVEL, ROW_GROUP_SIZE 122880);"
  record "parquet_rows_$t" "$(duckdb -noheader -list -c "SELECT count(*) FROM '$OUT/parquet/$t.parquet'")" ""
done
record parquet_build_s "$(echo "$(now) - $t0" | bc)" "zstd level $LEVEL"
record parquet_bytes "$(du -cb "$OUT"/parquet/*.parquet | tail -1 | cut -f1)" ""

# D. SQLite: tables, B-tree indexes for the API access paths, FTS5 over names
db="$OUT/cnpj.sqlite"
rm -f "$db"
t0=$(now)
duckdb -c "INSTALL sqlite; LOAD sqlite; ATTACH '$db' AS s (TYPE sqlite);
  CREATE TABLE s.empresas AS SELECT * FROM '$OUT/parquet/empresas.parquet';
  CREATE TABLE s.estabelecimentos AS SELECT * FROM '$OUT/parquet/estabelecimentos.parquet';
  CREATE TABLE s.socios AS SELECT * FROM '$OUT/parquet/socios.parquet';
  CREATE TABLE s.simples AS SELECT * FROM '$OUT/parquet/simples.parquet';"
record sqlite_load_s "$(echo "$(now) - $t0" | bc)" ""
record sqlite_tables_bytes "$(stat -c %s "$db")" ""
sql() { python3 -I -c 'import sqlite3,sys; c=sqlite3.connect(sys.argv[1]); c.executescript(sys.stdin.read()); c.commit()' "$db"; }
t0=$(now)
sql <<'SQL'
PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA cache_size=-2000000;
CREATE INDEX empresas_pk ON empresas(cnpj_basico);
CREATE INDEX estab_pk ON estabelecimentos(cnpj_basico, cnpj_ordem, cnpj_dv);
CREATE INDEX estab_uf_mun ON estabelecimentos(uf, municipio, situacao_cadastral);
CREATE INDEX estab_cnae ON estabelecimentos(cnae_fiscal_principal, situacao_cadastral);
CREATE INDEX socios_pk ON socios(cnpj_basico);
CREATE INDEX simples_pk ON simples(cnpj_basico);
SQL
record sqlite_btree_s "$(echo "$(now) - $t0" | bc)" ""
record sqlite_with_btree_bytes "$(stat -c %s "$db")" ""
t0=$(now)
sql <<'SQL'
PRAGMA journal_mode=OFF; PRAGMA synchronous=OFF; PRAGMA cache_size=-2000000;
CREATE VIRTUAL TABLE nomes USING fts5(razao_social, nome_fantasia, content='', tokenize='unicode61 remove_diacritics 2', prefix='2 3');
INSERT INTO nomes(rowid, razao_social, nome_fantasia)
  SELECT e.rowid, coalesce(m.razao_social, ''), e.nome_fantasia FROM estabelecimentos e LEFT JOIN empresas m USING (cnpj_basico);
SQL
record sqlite_fts_s "$(echo "$(now) - $t0" | bc)" ""
record sqlite_total_bytes "$(stat -c %s "$db")" ""
cat "$results"
