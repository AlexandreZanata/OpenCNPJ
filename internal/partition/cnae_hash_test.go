package partition

import "testing"

func TestCNAEHashBuckets(t *testing.T) {
	if CNAEHashBuckets != 4 {
		t.Fatalf("hash buckets = %d, want 4", CNAEHashBuckets)
	}
}

func TestMinCNAELeafPartitions(t *testing.T) {
	want := MinUFPartitions * CNAEHashBuckets
	if MinCNAELeafPartitions != want {
		t.Fatalf("leaf partitions = %d, want %d", MinCNAELeafPartitions, want)
	}
}

func TestCNAESubPartitionKey(t *testing.T) {
	if CNAESubPartitionKey != "cnae_part" {
		t.Fatalf("sub-partition key = %q", CNAESubPartitionKey)
	}
}
