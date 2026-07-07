-- EXPLAIN templates for CNAE+UF filtered search (plan 02 Phase 7).
-- Run: ./scripts/explain_cnae_uf_partition_pruning.sh
-- Expect: plan scans one UF branch hash leaf (e.g. estabelecimentos_sp_h2).

\echo '=== CNAE + UF + active estabelecimento (category browse gate) ==='
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT id FROM estabelecimentos
WHERE uf = 'SP' AND cnae_fiscal_principal = '4781400'
  AND situacao_cadastral = '02'
LIMIT 100;

\echo '=== CNAE + UF (RJ sample) ==='
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT e.id, e.cnpj_completo, e.cnae_fiscal_principal, e.uf
FROM estabelecimentos e
WHERE e.uf = 'RJ'
  AND e.cnae_fiscal_principal = '6201501'
  AND e.situacao_cadastral = '02'
ORDER BY e.id
LIMIT 21;

\echo '=== Leaf partition metadata (HASH under LIST) ==='
SELECT c.relname AS partition_name, pg_get_expr(c.relpartbound, c.oid) AS bound
FROM pg_inherits i
JOIN pg_class c ON c.oid = i.inhrelid
JOIN pg_class p ON p.oid = i.inhparent
WHERE p.relname = 'estabelecimentos_sp'
ORDER BY 1;

\echo '=== enable_partition_pruning ==='
SHOW enable_partition_pruning;
