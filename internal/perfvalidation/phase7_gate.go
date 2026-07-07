package perfvalidation

// Phase7MigrationFile adds HASH(cnae_fiscal_principal) sub-partitions under LIST(uf).
const Phase7MigrationFile = "migrations/000016_cnae_hash_subpartitions.up.sql"

// Phase7TopPartitionStrategy must appear in the migration DDL.
const Phase7TopPartitionStrategy = "PARTITION BY LIST (uf)"

// Phase7SubPartitionStrategy must appear in the migration DDL.
const Phase7SubPartitionStrategy = "PARTITION BY HASH (cnae_part)"

// Phase7ExplainScript validates CNAE+UF partition pruning via EXPLAIN.
const Phase7ExplainScript = "scripts/explain_cnae_uf_partition_pruning.sql"
