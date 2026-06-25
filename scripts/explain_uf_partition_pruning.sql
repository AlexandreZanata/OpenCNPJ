-- EXPLAIN templates for UF-filtered search (plan 02 Phase 6 — LIST(uf) pruning).
-- Run: ./scripts/explain_uf_partition_pruning.sh
-- Expect: plan scans a single uf partition (e.g. estabelecimentos_sp), not all children.

\echo '=== UF + active estabelecimento search (trigram) ==='
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT e.id, e.cnpj_completo, e.nome_fantasia, e.uf
FROM estabelecimentos e
WHERE e.nome_fantasia % 'PADARIA'
  AND e.uf = 'SP'
  AND e.situacao_cadastral = '02'
ORDER BY similarity(e.nome_fantasia, 'PADARIA') DESC, e.id ASC
LIMIT 21;

\echo '=== UF + CNAE filter ==='
EXPLAIN (ANALYZE, BUFFERS, FORMAT TEXT)
SELECT e.id, e.cnpj_completo, e.cnae_fiscal_principal, e.uf
FROM estabelecimentos e
WHERE e.uf = 'RJ'
  AND e.cnae_fiscal_principal = '6201501'
  AND e.situacao_cadastral = '02'
ORDER BY e.id
LIMIT 21;

\echo '=== Partition metadata (LIST on uf) ==='
SELECT c.relname AS partition_name, pg_get_expr(c.relpartbound, c.oid) AS bound
FROM pg_inherits i
JOIN pg_class c ON c.oid = i.inhrelid
JOIN pg_class p ON p.oid = i.inhparent
WHERE p.relname = 'estabelecimentos'
ORDER BY 1
LIMIT 15;

\echo '=== enable_partition_pruning ==='
SHOW enable_partition_pruning;
