package partition

// CNAEHashBuckets is the HASH modulus per UF partition (plan 02 Phase 7).
const CNAEHashBuckets = 4

// CNAESubPartitionKey is the HASH sub-partition column under each UF branch.
const CNAESubPartitionKey = "cnae_part"

// MinCNAELeafPartitions is UF branches (28 + EX + DEFAULT) × hash buckets.
const MinCNAELeafPartitions = MinUFPartitions * CNAEHashBuckets
