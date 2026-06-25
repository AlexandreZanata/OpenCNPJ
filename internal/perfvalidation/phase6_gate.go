package perfvalidation

// Phase6MigrationFile adds LIST(uf) partitioning on estabelecimentos.
const Phase6MigrationFile = "migrations/000014_uf_list_partitions.up.sql"

// Phase6PartitionStrategy must appear in the migration DDL.
const Phase6PartitionStrategy = "PARTITION BY LIST (uf)"

// Phase6ExplainScript validates UF partition pruning via EXPLAIN.
const Phase6ExplainScript = "scripts/explain_uf_partition_pruning.sql"
