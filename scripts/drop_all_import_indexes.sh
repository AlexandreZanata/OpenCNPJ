#!/usr/bin/env bash
set -euo pipefail

DB_USER="${DB_USER:-receita_user}"
DB_NAME="${DB_NAME:-receita_db}"

echo "==> Dropping UNIQUE constraints and secondary indexes on fact tables"
docker exec -i receita-postgres psql -v ON_ERROR_STOP=1 -U "$DB_USER" -d "$DB_NAME" <<'SQL'
DO $$
DECLARE r RECORD;
BEGIN
  FOR r IN
    SELECT t.relname AS tbl, c.conname AS con
    FROM pg_constraint c
    JOIN pg_class t ON c.conrelid = t.oid
    JOIN pg_namespace n ON t.relnamespace = n.oid
    WHERE n.nspname = 'public'
      AND t.relname IN ('empresas', 'estabelecimentos', 'socios', 'simples')
      AND c.contype = 'u'
  LOOP
    EXECUTE format('ALTER TABLE public.%I DROP CONSTRAINT IF EXISTS %I', r.tbl, r.con);
  END LOOP;

  FOR r IN
    SELECT i.relname AS index_name
    FROM pg_class t
    JOIN pg_index ix ON t.oid = ix.indrelid
    JOIN pg_class i ON i.oid = ix.indexrelid
    JOIN pg_namespace n ON n.oid = t.relnamespace
    WHERE n.nspname = 'public'
      AND t.relname IN ('empresas', 'estabelecimentos', 'socios', 'simples')
      AND NOT ix.indisprimary
  LOOP
    EXECUTE format('DROP INDEX IF EXISTS public.%I', r.index_name);
  END LOOP;
END $$;
SQL

echo "==> Indexes left (expect PK only):"
docker exec -i receita-postgres psql -U "$DB_USER" -d "$DB_NAME" -c \
  "SELECT t.relname, i.relname, ix.indisprimary FROM pg_class t
   JOIN pg_index ix ON t.oid = ix.indrelid
   JOIN pg_class i ON i.oid = ix.indexrelid
   JOIN pg_namespace n ON n.oid = t.relnamespace
   WHERE n.nspname='public' AND t.relname IN ('empresas','estabelecimentos','socios','simples')
   ORDER BY 1,2;"
