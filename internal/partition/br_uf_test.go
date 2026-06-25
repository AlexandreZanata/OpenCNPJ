package partition

import "testing"

func TestBrazilianUFCount(t *testing.T) {
	if len(BrazilianUFs) != 28 {
		t.Fatalf("uf count = %d, want 28", len(BrazilianUFs))
	}
}

func TestMinUFPartitions(t *testing.T) {
	if MinUFPartitions != 29 {
		t.Fatalf("min partitions = %d, want 29", MinUFPartitions)
	}
}

func TestEstabelecimentosPartitionKey(t *testing.T) {
	if EstabelecimentosPartitionKey != "uf" {
		t.Fatalf("partition key = %q", EstabelecimentosPartitionKey)
	}
}
